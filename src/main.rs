use colors_transform::{Color, Rgb};
use dotenv::dotenv;
use futures_util::io::BufReader;
use grammers_client::client::dialogs;
use grammers_session::PackedChat;
use grammers_tl_types::functions::channels::GetMessages;
use log;
use simple_logger::SimpleLogger;
use slint::{
    ComponentHandle, Image, Model, ModelExt, ModelRc, Rgb8Pixel, SharedPixelBuffer, VecModel,
};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufRead as _, Write as _};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::{thread, time::Duration};
use tokio::sync::Mutex;

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

slint::include_modules!();
#[tokio::main]
async fn main() {
    let pool = sqlx::sqlite::SqlitePool::connect("sqlite:crabgram.db")
        .await
        .unwrap();
    let (interface_sender, mut interface_receiver): (
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

    let main_window = MainWindow::new().unwrap();

    let mut pinned_dialog_count: i32 = 0;
    let dialogs: Arc<Mutex<Vec<grammers_client::types::Dialog>>> = Arc::new(Mutex::new(Vec::new()));
    let packed_chats: Arc<Mutex<HashMap<i64, PackedChat>>> = Arc::new(Mutex::new(HashMap::new()));
    let messages: Arc<Mutex<HashMap<i64, Vec<grammers_client::types::Message>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let selected_chat: Arc<Mutex<Option<i64>>> = Arc::new(Mutex::new(None));
    let selected_chat_messages: Arc<Mutex<Rc<VecModel<DisplayMessage>>>> =
        Arc::new(Mutex::new(Rc::new(VecModel::from(Vec::new()))));
    let interface_dialogs_model: Arc<Mutex<Vec<InterfaceDialog>>> =
        Arc::new(Mutex::new(Vec::new()));
    let mut callbacks_client_handle: Arc<Mutex<Option<grammers_client::client::Client>>> =
        Arc::new(Mutex::new(None));

    let selected_chat_clone = selected_chat.clone();
    let selected_chat_messages_clone = selected_chat_messages.clone();
    let messages_clone = messages.clone();
    let main_window_clone = main_window.clone_strong();
    let callbacks_client_handle_clone = callbacks_client_handle.clone();
    let packed_chats_clone = packed_chats.clone();
    main_window.on_select_chat(move |chat_id| {
        let mut selected_chat_lock =
            tokio::task::block_in_place(|| futures::executor::block_on(selected_chat_clone.lock()));
        let mut selected_chat_messages_lock = tokio::task::block_in_place(|| {
            futures::executor::block_on(selected_chat_messages_clone.lock())
        });
        *selected_chat_messages_lock = Rc::from(VecModel::from(Vec::new()));
        if let Ok(parsed_chat_id) = chat_id.parse::<i64>() {
            println!("Parsed chat");
            *selected_chat_lock = Some(parsed_chat_id);
            let mut messages_lock =
                tokio::task::block_in_place(|| futures::executor::block_on(messages_clone.lock()));
            let selected_dialog_messages =
                messages_lock.entry(parsed_chat_id).or_insert(Vec::new());
            if selected_dialog_messages.len() == 0 {
                println!("blocking callbacks");
                let mut callbacks_client_handle_lock = tokio::task::block_in_place(|| {
                    futures::executor::block_on(callbacks_client_handle_clone.lock())
                });
                println!("unblocking callbacks");
                if let Some(client_handle) = callbacks_client_handle_lock.clone() {
                    let packed_chats_lock = tokio::task::block_in_place(|| {
                        futures::executor::block_on(packed_chats_clone.lock())
                    });
                    if let Some(packed_chat) = packed_chats_lock.get(&parsed_chat_id) {
                        let mut messages_iter =
                            client_handle.iter_messages(*packed_chat).limit(100);
                        tokio::task::block_in_place(|| {
                            println!("Tokio blocked in place");
                            futures::executor::block_on(async {
                                messages_lock.insert(parsed_chat_id, Vec::new());
                                let mut cnt = 0;
                                while let Ok(maybe_message) = messages_iter.next().await {
                                    cnt += 1;
                                    println!("+message {}", cnt);
                                    if let Some(message) = maybe_message {
                                        selected_chat_messages_lock.insert(
                                            0,
                                            DisplayMessage {
                                                id: message.id().to_string().into(),
                                                text: message.text().into(),
                                                outgoing: message.outgoing()
                                                    || match message.sender() {
                                                        Some(chat) => {
                                                            chat.id() == message.chat().id()
                                                        }
                                                        None => false,
                                                    },
                                            },
                                        );
                                        messages_lock
                                            .entry(parsed_chat_id)
                                            .or_insert(Vec::new())
                                            .insert(0, message);
                                    } else {
                                        break;
                                    }
                                }
                            });
                            println!("Tokio unblocked in place");
                        });
                    }
                }
            } else {
                *selected_chat_messages_lock = Rc::from(VecModel::from(
                    selected_dialog_messages
                        .iter()
                        .map(|msg| DisplayMessage {
                            id: msg.id().to_string().into(),
                            text: msg.text().to_string().into(),
                        })
                        .collect::<Vec<DisplayMessage>>(),
                ));
            }
            drop(messages_lock);
        } else {
            *selected_chat_lock = None;
        }
        main_window_clone
            .set_selected_chat_messages(ModelRc::from(selected_chat_messages_lock.clone()));
        drop(selected_chat_lock);
        drop(selected_chat_messages_lock);
        println!("Unlocked everything");
    });
    let main_window_clone = main_window.clone_strong();
    // Creating dialogs
    let _ = slint::spawn_local(async move {
        let mut backend_client: grammers_client::client::Client;
        loop {
            if let Some(received) = interface_receiver.recv().await {
                println!("received some stuff");
                match received {
                    InterfaceMessage::InitialSetup(mut dialogs_getter, client) => {
                        let mut connection = pool.acquire().await.unwrap();
                        backend_client = client;
                        let mut callbacks_client_handle_lock = callbacks_client_handle.lock().await;
                        *callbacks_client_handle_lock = Some(backend_client.clone());
                        drop(callbacks_client_handle_lock);
                        let client_handle = backend_client.clone();
                        println!("setup");
                        println!("started iterating");
                        let main_window_clone_async = main_window_clone.clone_strong();
                        let packed_chats_clone = packed_chats.clone();
                        let dialogs_clone = dialogs.clone();
                        let interface_dialogs_model_clone = interface_dialogs_model.clone();
                        let interface_sender_spawned = interface_sender.clone();
                        let downloading_handle_clone = downloading_handle.clone();
                        let downloading_semaphore_clone = downloading_semaphore.clone();
                        let pool_spawned = pool.clone();
                        let _ = slint::spawn_local(async move {
                            while let Ok(maybe_dialog) = dialogs_getter.next().await {
                                let mut dialogs_lock = dialogs_clone.lock().await;
                                let mut interface_dialogs_model_lock =
                                    interface_dialogs_model_clone.lock().await;
                                if let Some(dialog) = maybe_dialog {
                                    let mut packed_chats_lock = packed_chats_clone.lock().await;
                                    packed_chats_lock
                                        .insert(dialog.chat().id(), dialog.chat().pack());
                                    drop(packed_chats_lock);
                                    let mut photo_path = String::new();
                                    let maybe_photo_id = match &dialog.chat {
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
                                        let downloadable = match dialog.clone().chat {
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
                                        let _ = match sqlx::query!(
                                            r#"SELECT big FROM ProfilePhoto WHERE big=false AND photo_id=?1"#,
                                            photo_id
                                            )
                                            .fetch_one(&mut *connection)
                                            .await
                                            {
                                                Ok(result) => {
                                                    photo_path = format!(
                                                        "cache/{}_{}",
                                                        photo_id,
                                                        if result.big.unwrap() { "big" } else { "small" }
                                                        )
                                                }
                                                Err(_) => {
                                                    photo_path = format!("cache/{}_small", photo_id);
                                                    let client_clone = client_handle.clone();
                                                    let pool_clone = pool_spawned.clone();
                                                    let photo_path_clone = photo_path.clone();
                                                    let photo_id = photo_id;
                                                    let interface_sender_downloading = interface_sender_spawned.clone();
                                                    let chat_id = dialog.chat.id();
                                                    let semaphore_clone = downloading_semaphore_clone.clone();
                                                    downloading_handle_clone.spawn(async move {
                                                        let _permit =
                                                            semaphore_clone.acquire().await.unwrap();
                                                        let _ = client_clone
                                                            .download_media(
                                                                &downloadable,
                                                                photo_path_clone.clone(),
                                                                )
                                                            .await;

                                                        let mut conn = pool_clone.acquire().await.unwrap();
                                                        let _ = sqlx::query!(
                                                            r#"INSERT INTO ProfilePhoto(photo_id, big) VALUES(?1, false)"#,
                                                            photo_id
                                                            )
                                                            .execute(&mut *conn)
                                                            .await;
                                                        let _ = interface_sender_downloading.send(
                                                            InterfaceMessage::ProfilePhotoUpdate(chat_id),
                                                            );
                                                    });
                                                }
                                            };
                                    }
                                    let mut profile_photo = Image::default();
                                    if !photo_path.is_empty() {
                                        let joined_path: &Path = &Path::new(&photo_path);
                                        if let Ok(opened_file) = File::open(joined_path) {
                                            if let Ok(mut reader_path) =
                                                image::io::Reader::open(joined_path)
                                            {
                                                reader_path.set_format(image::ImageFormat::Jpeg);
                                                if let Ok(opened_image) = reader_path.decode() {
                                                    let source_image = opened_image.into_rgba8();
                                                    let pixel_buffer =
                                                        slint::SharedPixelBuffer::clone_from_slice(
                                                            source_image.as_raw(),
                                                            source_image.width(),
                                                            source_image.height(),
                                                        );
                                                    profile_photo = Image::from_rgba8(pixel_buffer);
                                                }
                                            }
                                        }
                                    }
                                    let dialog_name = match dialog.chat() {
                                        grammers_client::types::Chat::User(user) => {
                                            user.full_name()
                                        }
                                        grammers_client::types::Chat::Group(group) => {
                                            group.title().into()
                                        }
                                        grammers_client::types::Chat::Channel(channel) => {
                                            channel.title().into()
                                        }
                                    };
                                    interface_dialogs_model_lock.push(InterfaceDialog {
                                        id: dialog.chat().id().to_string().into(),
                                        chat_name: dialog_name.into(),
                                        pinned: dialog.dialog.pinned(),
                                        profile_photo: profile_photo,
                                    });
                                    dialogs_lock.push(dialog);
                                    main_window_clone_async.set_dialog_list(ModelRc::from(
                                        Rc::new(VecModel::from(
                                            interface_dialogs_model_lock.clone(),
                                        )),
                                    ));
                                } else {
                                    break;
                                }
                            }
                        });
                        println!("Stopped iterating(huh)");
                    }
                    InterfaceMessage::NewMessage(message) => {
                        println!("I AKSHUALLY can receive messages");
                        let mut messages_lock = messages.lock().await;
                        let mut packed_chats_lock = packed_chats.lock().await;
                        packed_chats_lock.insert(message.chat().id(), message.chat().pack());
                        drop(packed_chats_lock);
                        if let Some(dialog_messages) = messages_lock.get_mut(&message.chat().id()) {
                            if !dialog_messages.is_empty() {
                                let chat_id = message.chat().id();
                                dialog_messages.push(message.clone());
                                println!("Trying to lock selected_chat(NewMessage)");
                                let selected_chat_lock = selected_chat.lock().await;
                                println!("Locked selected_chat(NewMessage)");
                                if Some(chat_id) == *selected_chat_lock {
                                    drop(selected_chat_lock);
                                    let selected_chat_messages_lock =
                                        selected_chat_messages.lock().await;
                                    selected_chat_messages_lock.push(DisplayMessage {
                                        id: message.id().to_string().into(),
                                        text: message.text().into(),
                                    });
                                    main_window_clone.set_selected_chat_messages(ModelRc::from(
                                        selected_chat_messages_lock.clone(),
                                    ));
                                } else {
                                    drop(selected_chat_lock);
                                }
                            }
                        }
                        drop(messages_lock);
                    }
                    _ => {}
                }
            }
        }
    });
    main_window.run().unwrap();
}

async fn async_main(
    interface_sender: tokio::sync::mpsc::UnboundedSender<InterfaceMessage>,
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
    let _ = interface_sender.send(InterfaceMessage::InitialSetup(dialogs, interface_handle));
    let interface_sender_clone = interface_sender.clone();

    while let Some(update) = update_handle.next_update().await? {
        //let client_handle = Arc::new(client.clone());
        match update {
            grammers_client::Update::NewMessage(message) => {
                let _ = interface_sender.send(InterfaceMessage::NewMessage(message));
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
    MessagesSetup(
        grammers_client::types::IterBuffer<
            grammers_tl_types::functions::messages::GetHistory,
            grammers_client::types::Message,
        >,
    ),
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
