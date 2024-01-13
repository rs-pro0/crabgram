use crate::interface::MainWindow;
use colors_transform::{Color, Rgb};
use dotenv::dotenv;
use log;
use simple_logger::SimpleLogger;
use slint::ComponentHandle;
use std::env;
use std::io::{self, BufRead as _, Write as _};
use std::sync::Arc;
use std::{thread, time::Duration};

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
    let pool = sqlx::sqlite::SqlitePool::connect("sqlite:crabgram.db")
        .await
        .unwrap();
    let (interface_sender, interface_receiver): (
        tokio::sync::mpsc::UnboundedSender<InterfaceMessage>,
        tokio::sync::mpsc::UnboundedReceiver<InterfaceMessage>,
    ) = tokio::sync::mpsc::unbounded_channel();
    let (downloading_shutdown_sender, downloading_shutdown_receiver): (
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Receiver<()>,
    ) = tokio::sync::oneshot::channel();
    let downloading_semaphore = Arc::new(tokio::sync::Semaphore::new(1));
    let (downloading_handle_sender, downloading_handle_recevier): (
        tokio::sync::oneshot::Sender<tokio::runtime::Handle>,
        tokio::sync::oneshot::Receiver<tokio::runtime::Handle>,
    ) = tokio::sync::oneshot::channel();
    // Spawning thread for downloading media
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _ = downloading_handle_sender.send(runtime.handle().clone());
        runtime.block_on(async {
            let _ = downloading_shutdown_receiver.await;
        });
    });
    let downloading_handle = downloading_handle_recevier.await.unwrap();
    let interface_sender_clone = interface_sender.clone();
    thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async_main(interface_sender_clone))
            .unwrap()
    });

    let background_colors: [colors_transform::Rgb; COLORS_NUMBER] = [
        Rgb::from_hex_str("#ff845e").unwrap(),
        Rgb::from_hex_str("#9ad164").unwrap(),
        Rgb::from_hex_str("#e5ca77").unwrap(),
        Rgb::from_hex_str("#5caffa").unwrap(),
        Rgb::from_hex_str("#b694f9").unwrap(),
        Rgb::from_hex_str("#ff8aac").unwrap(),
        Rgb::from_hex_str("#5bcbe3").unwrap(),
        Rgb::from_hex_str("#febb5b").unwrap(),
    ];
    MainWindow::new().unwrap().run().unwrap();
}

fn create_dialogs(
    dialogs: Vec<grammers_client::types::Dialog>,
    pinned_dialogs: Vec<grammers_client::types::Dialog>,
    dialog_elements: &mut Vec<crate::interface::Dialog>,
    dialogs_element: crate::interface,
    client_handle: Option<grammers_client::Client>,
    pool: sqlx::pool::Pool<sqlx::Sqlite>,
    interface_sender: glib::Sender<InterfaceMessage>,
    backround_colors: [Rgb; COLORS_NUMBER],
    downloading_handle: tokio::runtime::Handle,
    downloading_semaphore: Arc<tokio::sync::Semaphore>,
) {
    let mut connection = futures::executor::block_on(async { pool.acquire().await }).unwrap();
    dialogs_listbox.remove_all();
    for dialog in pinned_dialogs.iter().chain(dialogs.iter().rev()) {
        let row = gtk::ListBoxRow::new();
        let row_grid = gtk::Grid::builder()
            .column_spacing(10)
            .css_classes(vec!["dialog"])
            .hexpand(false)
            .build();
        let interface_sender_clone = interface_sender.clone();
        let chat_id = dialog.chat().id();
        let dialog_click_controller = gtk::GestureClick::new();
        dialog_click_controller.connect_pressed(move |_, _, _, _| {
            let _ = interface_sender_clone.send(InterfaceMessage::MakeChatActive(Some(chat_id)));
        });
        row.add_controller(dialog_click_controller);
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
                    r#"SELECT big FROM ProfilePhoto WHERE big=false AND photo_id=?1"#,
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
                    let pool_clone = pool.clone();
                    let photo_path_clone = photo_path.clone();
                    let photo_id = photo_id;
                    let interface_sender_clone = interface_sender.clone();
                    let chat_id = dialog.chat().id();
                    let semaphore_clone = downloading_semaphore.clone();
                    downloading_handle.spawn(async move {
                        let _permit = semaphore_clone.acquire().await.unwrap();
                        let _ = client_clone
                            .download_media(&downloadable, photo_path_clone.clone())
                            .await;

                        let mut conn = pool_clone.acquire().await.unwrap();
                        let _ = sqlx::query!(
                            r#"INSERT INTO ProfilePhoto(photo_id, big) VALUES(?1, false)"#,
                            photo_id
                        )
                        .execute(&mut *conn)
                        .await;
                        let _ = interface_sender_clone
                            .send(InterfaceMessage::ProfilePhotoUpdate(chat_id));
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
        profile_picture.set_draw_func(move |area, context, _, _| {
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
                let _ = context.paint();
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
                let _ = context.fill();
                context.set_source_rgb(1.0, 1.0, 1.0);
                context.set_font_size(24.0);
                let text_extents = context.text_extents(&first_letter.to_string()).unwrap();
                context.move_to(
                    area_width / 2.0 - text_extents.x_bearing() - text_extents.width() / 2.0,
                    area_height / 2.0 - text_extents.y_bearing() - text_extents.height() / 2.0,
                );
                let _ = context.show_text(&first_letter.to_string());
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
            row.set_data("messages", Vec::<grammers_client::types::Message>::new());
        }
        dialog_elements.push(row);
    }
}
async fn async_main(
    sender: tokio::sync::mpsc::UnboundedSender<InterfaceMessage>,
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
    let _ = sender.send(InterfaceMessage::InitialSetup(dialogs, interface_handle));

    while let Some(update) = update_handle.next_update().await? {
        //let client_handle = Arc::new(client.clone());
        match update {
            grammers_client::Update::NewMessage(message) => {
                let _ = sender.send(InterfaceMessage::NewMessage(message));
            }
            _ => {}
        }
    }

    if sign_out {
        drop(client.sign_out_disconnect().await);
    }

    Ok(())
}

enum InterfaceMessage {
    NewMessage(grammers_client::types::Message),
    InitialSetup(
        grammers_client::types::IterBuffer<
            grammers_tl_types::functions::messages::GetDialogs,
            grammers_client::types::Dialog,
        >,
        grammers_client::Client,
    ),
    ProfilePhotoUpdate(i64), // This i64 is id of chat where image should be updated
    MakeChatActive(Option<i64>), // This i64 is id of chat which should become active
    SendMessage,
}
