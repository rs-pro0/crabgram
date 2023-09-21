use colors_transform::{Color, Rgb};
use dotenv::dotenv;
use gtk::glib;
use gtk::prelude::*;
use log;
use simple_logger::SimpleLogger;
use std::env;
use std::io::{self, BufRead as _, Write as _};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime;

const SESSION_FILE: &str = "sess.session";
const COLORS_NUMBER: usize = 8;

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
    let pool = sqlx::sqlite::SqlitePool::connect("sqlite:crabgram.db")
        .await
        .unwrap();
    let mut connection = pool.acquire().await.unwrap();
    let tokio_handle = tokio::runtime::Handle::current();
    let query_result = sqlx::query!(r#"SELECT * FROM Photo"#)
        .fetch_all(&mut *connection)
        .await
        .unwrap();
    let application = gtk::Application::builder()
        .application_id("crabgram")
        .build();
    application.connect_startup(|_| load_css());
    application.connect_activate(build_ui);
    let interface_sender_clone = interface_sender.clone();
    thread::spawn(move || {
        runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async_main(interface_sender_clone))
            .unwrap()
    });
    let mut dialog_count: i32 = 0;
    let mut pinned_dialog_count: i32 = 0;
    let dialog_element_list: Vec<gtk::ListBoxRow> = Vec::new();
    let dialog_element_list_mutex: Mutex<Vec<gtk::ListBoxRow>> = Mutex::new(dialog_element_list);
    let application_clone = application.clone();
    let mut interface_handle: Option<grammers_client::Client> = None;

    let BACKGROUND_COLORS: [colors_transform::Rgb; COLORS_NUMBER] = [
        Rgb::from_hex_str("#ff845e").unwrap(),
        Rgb::from_hex_str("#9ad164").unwrap(),
        Rgb::from_hex_str("#e5ca77").unwrap(),
        Rgb::from_hex_str("#5caffa").unwrap(),
        Rgb::from_hex_str("#b694f9").unwrap(),
        Rgb::from_hex_str("#ff8aac").unwrap(),
        Rgb::from_hex_str("#5bcbe3").unwrap(),
        Rgb::from_hex_str("#febb5b").unwrap(),
    ];

    interface_receiver.attach(None, move |msg| {
        let interface_sender_clone = interface_sender.clone();
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
                //println!("Received {}", message.text());
                let dialog_element_list_lock = dialog_element_list_mutex.lock().unwrap();
                for dialog in dialog_element_list_lock.iter() {
                    /*let label: gtk::Label =
                    unsafe { dialog.child().unwrap().last_child().unwrap().unsafe_cast() };*/
                    //println!("{}", label.text());
                    let data = unsafe {
                        dialog
                            .data::<grammers_client::types::Dialog>("dialog")
                            .unwrap()
                            .as_mut()
                    };
                    if data.chat().id() == message.chat().id() {
                        let dialogs_listbox_clone = dialogs_listbox.clone();
                        let dialog_grid: gtk::Grid =
                            unsafe { dialog.child().unwrap().unsafe_cast() };
                        let message_label: gtk::Label =
                            unsafe { dialog_grid.child_at(1, 1).unwrap().unsafe_cast() };
                        match &data.last_message {
                            Some(last_message) => {
                                if &message.id() > &last_message.id() {
                                    data.last_message = Some(message.clone());
                                }
                            }
                            None => {
                                data.last_message = Some(message.clone());
                            }
                        }
                        match message_labeler(&data.last_message) {
                            Some(text) => message_label.set_text(&text),
                            None => {}
                        }
                        if !data.dialog.pinned() {
                            dialogs_listbox_clone.remove(dialog);
                            dialogs_listbox_clone.insert(dialog, pinned_dialog_count);
                        }
                        break;
                    }
                }
                /*println!(
                    "got message {} from {}",
                    message.text(),
                    message.chat().name()
                );*/
            }
            InterfaceMessage::InitialSetup(mut dialogs, client_handle) => {
                //let interface_handle_arc = Arc::clone(&interface_handle);
                //*interface_handle_arc = Some(client_handle);
                interface_handle = Some(client_handle);
                let mut dialog_list: Vec<grammers_client::types::Dialog> = Vec::new();
                let mut pinned_dialog_list: Vec<grammers_client::types::Dialog> = Vec::new();
                let mut connection = None;
                futures::executor::block_on(async {
                    connection = Some(pool.acquire().await.unwrap());
                    while let Some(dialog) = dialogs.next().await.unwrap() {
                        if dialog.dialog.pinned() {
                            pinned_dialog_list.push(dialog);
                        } else {
                            dialog_list.push(dialog);
                        }
                    }
                });
                dialog_list.reverse();
                pinned_dialog_count = pinned_dialog_list.len() as i32;
                let mut dialog_element_list_lock = dialog_element_list_mutex.lock().unwrap();
                create_dialogs(
                    dialog_list,
                    pinned_dialog_list,
                    &mut dialog_element_list_lock,
                    dialogs_listbox.clone(),
                    interface_handle.clone(),
                    pool.clone(),
                    tokio_handle.clone(),
                    interface_sender_clone,
                    BACKGROUND_COLORS,
                );
            }
            InterfaceMessage::ImageUpdate(chat_id) => {
                let dialog_element_list_lock = dialog_element_list_mutex.lock().unwrap();
                for dialog in dialog_element_list_lock.iter() {
                    let data = unsafe {
                        dialog
                            .data::<grammers_client::types::Dialog>("dialog")
                            .unwrap()
                            .as_mut()
                    };
                    if data.chat().id() == chat_id {
                        let dialog_grid: gtk::Grid =
                            unsafe { dialog.child().unwrap().unsafe_cast() };
                        let dialog_image: gtk::DrawingArea =
                            unsafe { dialog_grid.child_at(0, 0).unwrap().unsafe_cast() };
                        dialog_image.queue_draw();
                        break;
                    }
                }
            }
        }
        glib::ControlFlow::Continue
    });
    application.run();
}

fn message_labeler(message: &Option<grammers_client::types::Message>) -> Option<String> {
    match message {
        Some(msg) => {
            let txt = msg.text().trim().replace('\n', " ");
            Some(txt)
        }
        None => None,
    }
}

fn create_dialogs(
    dialogs: Vec<grammers_client::types::Dialog>,
    pinned_dialogs: Vec<grammers_client::types::Dialog>,
    dialog_elements: &mut Vec<gtk::ListBoxRow>,
    dialogs_listbox: gtk::ListBox,
    client_handle: Option<grammers_client::Client>,
    pool: sqlx::pool::Pool<sqlx::Sqlite>,
    tokio_handle: tokio::runtime::Handle,
    interface_sender: glib::Sender<InterfaceMessage>,
    backround_colors: [Rgb; COLORS_NUMBER],
) {
    let mut connection = futures::executor::block_on(async { pool.acquire().await }).unwrap();
    dialogs_listbox.remove_all();
    for dialog in pinned_dialogs.iter().rev().chain(dialogs.iter().rev()) {
        let row = gtk::ListBoxRow::new();
        let row_grid = gtk::Grid::builder()
            .column_spacing(10)
            .css_classes(vec!["dialog"])
            .hexpand(false)
            .build();
        let chat = dialog.chat.clone();
        let label_text = match chat.clone() {
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
        let mut photo_path = String::new();
        let maybe_photo_id = match &chat {
            grammers_client::types::Chat::User(user) => {
                if let Some(photo) = user.photo() {
                    Some(photo.photo_id)
                } else {
                    None
                }
            }
            grammers_client::types::Chat::Group(group) => {
                if let Some(photo) = group.photo() {
                    Some(photo.photo_id)
                } else {
                    None
                }
            }
            grammers_client::types::Chat::Channel(channel) => {
                if let Some(photo) = channel.photo() {
                    Some(photo.photo_id)
                } else {
                    None
                }
            }
        };
        if let Some(photo_id) = maybe_photo_id {
            let downloadable = match chat {
                grammers_client::types::Chat::User(user) => {
                    let photo = user.photo().unwrap();
                    grammers_client::types::Downloadable::UserProfilePhoto(
                        grammers_client::types::UserProfilePhoto {
                            big: false,
                            peer: user.pack().to_input_peer(),
                            photo: photo.clone(),
                        },
                    )
                }
                grammers_client::types::Chat::Group(group) => {
                    let photo = group.photo().unwrap();
                    grammers_client::types::Downloadable::ChatPhoto(
                        grammers_client::types::ChatPhoto {
                            big: false,
                            peer: group.pack().to_input_peer(),
                            photo: photo.clone(),
                        },
                    )
                }
                grammers_client::types::Chat::Channel(channel) => {
                    let photo = channel.photo().unwrap();
                    grammers_client::types::Downloadable::ChatPhoto(
                        grammers_client::types::ChatPhoto {
                            big: false,
                            peer: channel.pack().to_input_peer(),
                            photo: photo.clone(),
                        },
                    )
                }
            };
            let _ = match futures::executor::block_on(async {
                sqlx::query!(
                    r#"SELECT big FROM Photo WHERE big=false AND photo_id=?1"#,
                    photo_id
                )
                .fetch_one(&mut *connection)
                .await
            }) {
                Ok(result) => {
                    photo_path = format!(
                        "cache/{}_{}",
                        photo_id,
                        if result.big.unwrap() { "big" } else { "small" }
                    )
                }
                Err(_) => {
                    photo_path = format!("cache/{}_small", photo_id);
                    let client_clone = client_handle.clone().unwrap();
                    let mut conn =
                        futures::executor::block_on(async { pool.acquire().await.unwrap() });
                    let photo_path_clone = photo_path.clone();
                    let photo_id = photo_id;
                    let tokio_handle_clone = tokio_handle.clone();
                    let interface_sender_clone = interface_sender.clone();
                    let chat_id = dialog.chat().id();
                    thread::spawn(move || {
                        tokio_handle_clone.spawn(async move {
                            let _ = client_clone
                                .download_media(&downloadable, photo_path_clone)
                                .await;
                            let _ = sqlx::query!(
                                r#"INSERT INTO Photo(photo_id, big) VALUES(?1, false)"#,
                                photo_id
                            )
                            .execute(&mut *conn)
                            .await;
                            interface_sender_clone.send(InterfaceMessage::ImageUpdate(chat_id));
                        });
                    });
                }
            };
        }
        let dialog_name = gtk::Label::builder()
            .label(label_text.clone())
            .halign(gtk::Align::Start)
            .css_classes(vec!["dialog_label"])
            .build();
        let profile_picture = gtk::DrawingArea::builder()
            .css_classes(vec!["profile_picture"])
            .build();
        let photopath_clone = photo_path.clone();
        let color_index = (dialog.chat().id() % 7) as usize;
        let color = backround_colors[color_index];
        let first_letter = label_text.chars().nth(0).unwrap();
        profile_picture.set_draw_func(move |area, context, width, height| {
            let parent_height = area.parent().unwrap().height();
            area.set_size_request(parent_height, -1);
            let area_width = area.width() as f64;
            let area_height = area.height() as f64;
            if let Ok(pixbuf) = gdk_pixbuf::Pixbuf::from_file(Path::new(&photopath_clone)) {
                let width = pixbuf.width() as f64;
                let height = pixbuf.height() as f64;
                context.scale(area_width / width, area_height / height);
                context.set_source_pixbuf(&pixbuf, 0.0, 0.0);
                context.arc(
                    width / 2.0,
                    height / 2.0,
                    width / 2.0,
                    0.0,
                    2.0 * std::f64::consts::PI,
                );
                context.clip();
                context.paint();
            } else {
                context.set_source_rgb(
                    color.get_red() as f64 / 255.0,
                    color.get_green() as f64 / 255.0,
                    color.get_blue() as f64 / 255.0,
                );
                context.arc(
                    area_width / 2.0,
                    area_height / 2.0,
                    area_width / 2.0,
                    0.0,
                    2.0 * std::f64::consts::PI,
                );
                context.fill();
                context.set_source_rgb(1.0, 1.0, 1.0);
                context.set_font_size(24.0);
                let text_extents = context.text_extents(&first_letter.to_string()).unwrap();
                context.move_to(
                    area_width / 2.0 - text_extents.x_bearing() - text_extents.width() / 2.0,
                    area_height / 2.0 - text_extents.y_bearing() - text_extents.height() / 2.0,
                );
                context.show_text(&first_letter.to_string());
            }
        });
        let mut message_label_builder = gtk::Label::builder()
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .halign(gtk::Align::Start);
        match message_labeler(&dialog.last_message) {
            Some(text) => message_label_builder = message_label_builder.label(text),
            None => {}
        }
        let message_label = message_label_builder.build();
        row_grid.attach(&profile_picture, 0, 0, 1, 2);
        row_grid.attach(&dialog_name, 1, 0, 1, 1);
        row_grid.attach(&message_label, 1, 1, 1, 1);
        row.set_child(Some(&row_grid));
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
    InitialSetup(
        grammers_client::types::IterBuffer<
            grammers_tl_types::functions::messages::GetDialogs,
            grammers_client::types::Dialog,
        >,
        grammers_client::Client,
    ),
    ImageUpdate(i64),
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../styles/main.css"));

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
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Always)
        .child(&dialogs)
        .css_classes(vec!["dialogs"])
        .vexpand(true)
        .build();
    grid.attach(&scrolled_window, 0, 0, 1, 1);
    grid.attach(&main_window, 1, 0, 1, 1);
    let listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    dialogs.append(&listbox);

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

    let mut sign_out = false;

    if !client.is_authorized().await? {
        println!("Signing in...");
        let phone = prompt("Enter your phone number (international format): ")?;
        let token = client.request_login_code(&phone).await?;
        let code = prompt("Enter the code you received: ")?;
        let signed_in = client.sign_in(&token, &code).await;
        match signed_in {
            Err(grammers_client::SignInError::PasswordRequired(password_token)) => {
                // This is a login from example of grammers, because I don't want to implement
                // login right now(I will do it in future)
                // TODO: implement login system
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

    let update_handle = client.clone();
    let interface_handle = client.clone();
    let dialogs = interface_handle.iter_dialogs();
    sender.send(InterfaceMessage::InitialSetup(dialogs, interface_handle));

    while let Some(update) = update_handle.next_update().await? {
        //let client_handle = Arc::new(client.clone());
        match update {
            grammers_client::Update::NewMessage(message) => {
                //println!("Sent message: {}", message.text());
                sender.send(InterfaceMessage::NewMessage(message));
            }
            _ => {}
        }
    }

    if sign_out {
        drop(client.sign_out_disconnect().await);
    }

    Ok(())
}
