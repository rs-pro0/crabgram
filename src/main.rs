use dotenv::dotenv;
use glib::{clone, MainContext, Priority};
use gtk::glib;
use gtk::prelude::*;
use log;
use simple_logger::SimpleLogger;
use std::env;
use std::io::{self, BufRead as _, Write as _};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::{runtime, task};

const SESSION_FILE: &str = "sess.session";

fn prompt(message: &str) -> Result<String, Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(message.as_bytes())?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut line = String::new();
    stdin.read_line(&mut line)?;
    Ok(line)
}

#[tokio::main]
async fn main() {
    let (sender, receiver): (
        glib::Sender<InterfaceMessage>,
        glib::Receiver<InterfaceMessage>,
    ) = glib::MainContext::channel(glib::Priority::DEFAULT);
    let application = gtk::Application::builder()
        .application_id("crabgram")
        .build();
    application.connect_startup(|_| load_css());
    application.connect_activate(build_ui);
    thread::spawn(move || {
        runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async_main(sender))
            .unwrap()
    });
    let application_clone = application.clone();
    receiver.attach(None, move |msg| {
        match msg {
            InterfaceMessage::NewMessage(message) => {
                println!(
                    "got message {} from {}",
                    message.text(),
                    message.chat().name()
                );
            }
            InterfaceMessage::Dialogs(dialogs) => {
                let grid_base = application_clone.windows()[0].child().unwrap();
                let grid_base_grid: gtk::Grid = unsafe { grid_base.unsafe_cast() };
                let dialogs_element: gtk::Box =
                    unsafe { grid_base_grid.child_at(0, 0).unwrap().unsafe_cast() };
                let dialogs_listbox: gtk::ListBox =
                    unsafe { dialogs_element.first_child().unwrap().unsafe_cast() };

                for dialog in dialogs {
                    let row = gtk::ListBoxRow::new();
                    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    row_box.add_css_class("dialog");
                    let label = gtk::Label::new(Some(dialog.chat.name()));
                    row_box.append(&label);
                    row.set_child(Some(&row_box));
                    dialogs_listbox.append(&row);
                }
            }
        }
        glib::ControlFlow::Continue
    });
    application.run();
}

enum InterfaceMessage {
    NewMessage(grammers_client::types::Message),
    Dialogs(Vec<grammers_client::types::Dialog>),
}

fn load_css() {
    // Load the CSS file and add it to the provider
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../styles/main.css"));

    // Add the provider to the default screen
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title(Some("Crabgram"));
    window.set_default_size(350, 70);

    let grid = gtk::Grid::new();
    let main_window = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    main_window.add_css_class("main_window");
    main_window.set_hexpand(true);
    let dialogs = gtk::Box::new(gtk::Orientation::Vertical, 8);
    dialogs.add_css_class("dialogs");
    dialogs.set_vexpand(true);
    grid.attach(&dialogs, 0, 0, 1, 1);
    grid.attach(&main_window, 1, 0, 1, 1);
    let listbox = gtk::ListBox::new();
    dialogs.append(&listbox);
    listbox.set_selection_mode(gtk::SelectionMode::None);

    window.set_child(Some(&grid));

    window.present();
}

async fn async_main(
    sender: glib::Sender<InterfaceMessage>,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    SimpleLogger::new()
        .with_level(log::LevelFilter::Error)
        .init()
        .unwrap();
    let api_id: i32 = env::var("api_id").unwrap().parse().unwrap();
    let api_hash: String = env::var("api_hash").unwrap();

    println!("Connecting to Telegram...");
    let client = grammers_client::Client::connect(grammers_client::Config {
        session: grammers_session::Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.to_string(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    // If we can't save the session, sign out once we're done.
    let mut sign_out = false;

    if !client.is_authorized().await? {
        println!("Signing in...");
        let phone = prompt("Enter your phone number (international format): ")?;
        let token = client.request_login_code(&phone, api_id, &api_hash).await?;
        let code = prompt("Enter the code you received: ")?;
        let signed_in = client.sign_in(&token, &code).await;
        match signed_in {
            Err(grammers_client::SignInError::PasswordRequired(password_token)) => {
                // Note: this `prompt` method will echo the password in the console.
                //       Real code might want to use a better way to handle this.
                let hint = password_token.hint().unwrap_or("None");
                let prompt_message = format!("Enter the password (hint {}): ", &hint);
                let password = prompt(prompt_message.as_str())?;

                client
                    .check_password(password_token, password.trim())
                    .await?;
            }
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        };
        println!("Signed in!");
        match client.session().save_to_file(SESSION_FILE) {
            Ok(_) => {}
            Err(e) => {
                println!(
                    "NOTE: failed to save the session, will sign out when done: {}",
                    e
                );
                sign_out = true;
            }
        }
    }

    // Obtain a `ClientHandle` to perform remote calls while `Client` drives the connection.
    //
    // This handle can be `clone()`'d around and freely moved into other tasks, so you can invoke
    // methods concurrently if you need to. While you do this, the single owned `client` is the
    // one that communicates with the network.
    //
    // The design's annoying to use for trivial sequential tasks, but is otherwise scalable.
    let main_handle = client.clone();
    let mut dialogs = main_handle.iter_dialogs();
    let mut dialogs_list: Vec<grammers_client::types::Dialog> = Vec::new();
    while let Some(dialog) = dialogs.next().await.unwrap() {
        dialogs_list.push(dialog);
    }
    sender.send(InterfaceMessage::Dialogs(dialogs_list));

    while let Some(update) = main_handle.next_update().await? {
        let client_handle = Arc::new(client.clone());
        match update {
            grammers_client::Update::NewMessage(message) => {
                sender.send(InterfaceMessage::NewMessage(message));
            }
            _ => {}
        }
    }

    if sign_out {
        // TODO revisit examples and get rid of "handle references" (also, this panics)
        drop(client.sign_out_disconnect().await);
    }

    Ok(())
}
