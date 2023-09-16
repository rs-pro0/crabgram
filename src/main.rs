use dotenv::dotenv;
use gtk::glib;
use gtk::prelude::*;
use log;
use simple_logger::SimpleLogger;
use std::env;
use std::io::{self, BufRead as _, Write as _};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime;

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
    let (interface_sender, interface_receiver): (
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
            .block_on(async_main(interface_sender))
            .unwrap()
    });
    let mut dialog_global_list: Vec<grammers_client::types::Dialog> = Vec::new();
    let mut dialog_pinned_global_list: Vec<grammers_client::types::Dialog> = Vec::new();
    let dialog_element_list: Vec<gtk::ListBoxRow> = Vec::new();
    let dialog_element_list_mutex: Arc<Mutex<Vec<gtk::ListBoxRow>>> =
        Arc::new(Mutex::new(dialog_element_list));
    let application_clone = application.clone();

    interface_receiver.attach(None, move |msg| {
        let grid_base = application_clone.windows()[0].child().unwrap();
        let grid_base_grid: gtk::Grid = unsafe { grid_base.unsafe_cast() };
        let scrolled_window: gtk::ScrolledWindow =
            unsafe { grid_base_grid.child_at(0, 0).unwrap().unsafe_cast() };
        let dialogs_element: gtk::Box = unsafe {
            scrolled_window
                .child()
                .unwrap()
                .first_child()
                .unwrap()
                .unsafe_cast()
        };
        let dialogs_listbox: gtk::ListBox =
            unsafe { dialogs_element.first_child().unwrap().unsafe_cast() };
        match msg {
            InterfaceMessage::NewMessage(message) => {
                let dialog_arc_clone = Arc::clone(&dialog_element_list_mutex);
                let dialog_element_list_lock = dialog_arc_clone.lock().unwrap();
                for dialog in dialog_element_list_lock.iter() {
                    /*let label: gtk::Label =
                    unsafe { dialog.child().unwrap().last_child().unwrap().unsafe_cast() };*/
                    //println!("{}", label.text());
                    if let Some(mut dt) =
                        unsafe { dialog.data::<grammers_client::types::Dialog>("dialog") }
                    {
                        let data = unsafe { dt.as_mut() };
                        if data.chat().id() == message.chat().id() {
                            let dialogs_listbox_clone = dialogs_listbox.clone();
                            dialogs_listbox_clone.remove(dialog);
                            if data.dialog.pinned() {
                                dialogs_listbox_clone.insert(dialog, 0);
                            } else {
                                dialogs_listbox_clone
                                    .insert(dialog, dialog_pinned_global_list.len() as i32);
                            }
                        }
                    } else {
                        println!("empty data");
                    }
                }
                if message.pinned() {
                    for dialog_index in 0..dialog_pinned_global_list.len() {
                        if dialog_pinned_global_list[dialog_index].chat().id()
                            == message.chat().id()
                        {
                            dialog_pinned_global_list[dialog_index].last_message =
                                Some(message.clone());
                            dialog_pinned_global_list
                                .push(dialog_global_list[dialog_index].clone());
                            dialog_pinned_global_list.remove(dialog_index);
                            break;
                        }
                    }
                } else {
                    for dialog_index in 0..dialog_global_list.len() {
                        if dialog_global_list[dialog_index].chat().id() == message.chat().id() {
                            dialog_global_list[dialog_index].last_message = Some(message.clone());
                            dialog_global_list.push(dialog_global_list[dialog_index].clone());
                            dialog_global_list.remove(dialog_index);
                            break;
                        }
                    }
                }
                /*println!(
                    "got message {} from {}",
                    message.text(),
                    message.chat().name()
                );*/
            }
            InterfaceMessage::Dialogs(mut dialogs) => {
                dialog_global_list.clear();
                futures::executor::block_on(async {
                    while let Some(dialog) = dialogs.next().await.unwrap() {
                        if dialog.dialog.pinned() {
                            dialog_pinned_global_list.push(dialog);
                        } else {
                            dialog_global_list.push(dialog);
                        }
                    }
                });
                dialog_global_list.reverse();
                dialog_pinned_global_list.reverse();
                let dialog_arc_clone = Arc::clone(&dialog_element_list_mutex);
                let mut dialog_element_list_lock = dialog_arc_clone.lock().unwrap();
                create_dialogs(
                    &dialog_global_list,
                    &dialog_pinned_global_list,
                    &mut dialog_element_list_lock,
                    dialogs_listbox.clone(),
                );
            }
        }
        glib::ControlFlow::Continue
    });
    application.run();
}

fn create_dialogs(
    dialogs: &Vec<grammers_client::types::Dialog>,
    pinned_dialogs: &Vec<grammers_client::types::Dialog>,
    dialog_elements: &mut Vec<gtk::ListBoxRow>,
    dialogs_listbox: gtk::ListBox,
) {
    dialogs_listbox.remove_all();
    for dialog in pinned_dialogs.iter().rev().chain(dialogs.iter().rev()) {
        let row = gtk::ListBoxRow::new();
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row_box.add_css_class("dialog");
        let chat = dialog.chat.clone();
        let label_text = match chat {
            grammers_client::types::Chat::User(user) => {
                if user.deleted() {
                    "Deleted account".to_string()
                } else if user.is_self() {
                    "Saved Messages".to_string()
                } else {
                    user.full_name()
                }
            }
            _ => chat.name().to_string(),
        };
        let label = gtk::Label::new(Some(&label_text));
        row_box.append(&label);
        row.set_child(Some(&row_box));
        dialogs_listbox.append(&row);
        unsafe {
            row.set_data("dialog", dialog.clone());
        }
        dialog_elements.push(row);
    }
}

enum InterfaceMessage {
    NewMessage(grammers_client::types::Message),
    //Dialogs(Vec<grammers_client::types::Dialog>),
    Dialogs(
        grammers_client::types::IterBuffer<
            grammers_tl_types::functions::messages::GetDialogs,
            grammers_client::types::Dialog,
        >,
    ),
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
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Always);
    scrolled_window.set_child(Some(&dialogs));
    dialogs.add_css_class("dialogs");
    dialogs.set_vexpand(true);
    grid.attach(&scrolled_window, 0, 0, 1, 1);
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
    let dialogs = main_handle.iter_dialogs();
    sender.send(InterfaceMessage::Dialogs(dialogs));

    while let Some(update) = main_handle.next_update().await? {
        //let client_handle = Arc::new(client.clone());
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
