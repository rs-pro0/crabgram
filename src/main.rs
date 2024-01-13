use std::{thread, time::Duration};
pub mod interface;

#[tokio::main]
async fn main() {
    thread::spawn(|| {
        MainWindow::new().unwrap().run().unwrap();
    });
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
/*use colors_transform::{Color, Rgb};
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
    let application = gtk::Application::builder()
        .application_id("crabgram")
        .build();
    application.connect_startup(|_| load_css());
    let interface_sender_clone = interface_sender.clone();
    application
        .connect_activate(move |application| build_ui(application, interface_sender_clone.clone()));
    let interface_sender_clone = interface_sender.clone();
    thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async_main(interface_sender_clone))
            .unwrap()
    });
    let mut pinned_dialog_count: i32 = 0;
    let dialog_element_list: Vec<gtk::ListBoxRow> = Vec::new();
    let dialog_element_list_mutex: Mutex<Vec<gtk::ListBoxRow>> = Mutex::new(dialog_element_list);
    let application_clone = application.clone();
    let mut interface_handle: Option<grammers_client::Client> = None;
    let active_chat: Mutex<Option<i64>> = Mutex::new(None);

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
                let dialog_element_list_lock = dialog_element_list_mutex.lock().unwrap();
                for dialog in dialog_element_list_lock.iter() {
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
                        let mut messages_data = unsafe {
                            dialog
                                .steal_data::<Vec<grammers_client::types::Message>>("messages")
                                .unwrap()
                        };
                        let mut index = messages_data.len();
                        loop {
                            if index == 0 || messages_data[index - 1].id() < message.id() {
                                if messages_data.len() != 0 {
                                    messages_data.insert(index, message.clone());
                                    let active_chat_lock = active_chat.lock().unwrap();
                                    if *active_chat_lock == Some(data.chat().id()) {
                                        let main_window: gtk::Grid = unsafe {
                                            grid_base_grid.child_at(1, 0).unwrap().unsafe_cast()
                                        };
                                        let message_scrolled_window: gtk::ScrolledWindow = unsafe {
                                            main_window.child_at(0, 0).unwrap().unsafe_cast()
                                        };
                                        let message_view: gtk::ListBox = unsafe {
                                            message_scrolled_window
                                                .first_child()
                                                .unwrap()
                                                .first_child()
                                                .unwrap()
                                                .unsafe_cast()
                                        };
                                        let (
                                            downloading_handle_clone,
                                            downloading_semaphore_clone,
                                            client_handle_clone,
                                        ) = match message.photo() {
                                            Some(_) => (
                                                Some(downloading_handle.clone()),
                                                Some(downloading_semaphore.clone()),
                                                interface_handle.clone(),
                                            ),
                                            None => (None, None, None),
                                        };
                                        let message_row = create_message_row(
                                            &message,
                                            downloading_handle_clone,
                                            downloading_semaphore_clone,
                                            client_handle_clone,
                                            pool.clone(),
                                        );
                                        message_view.insert(&message_row, index as i32);
                                        if message.outgoing() {
                                            scroll_down(message_scrolled_window);
                                        }
                                    }
                                }
                                break;
                            } else if messages_data[index - 1].id() == message.id() {
                                break;
                            }
                            index -= 1;
                        }
                        unsafe {
                            dialog.set_data("messages", messages_data);
                        }

                        if !data.dialog.pinned() {
                            dialogs_listbox_clone.remove(dialog);
                            dialogs_listbox_clone.insert(dialog, pinned_dialog_count);
                        }
                        break;
                    }
                }
            }
            InterfaceMessage::InitialSetup(mut dialogs, client_handle) => {
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
                    interface_sender_clone,
                    background_colors,
                    downloading_handle.clone(),
                    downloading_semaphore.clone(),
                );
            }
            InterfaceMessage::ProfilePhotoUpdate(chat_id) => {
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
            InterfaceMessage::MakeChatActive(chat_id) => {
                let dialog_element_list_lock = dialog_element_list_mutex.lock().unwrap();
                let (mut found_previous, mut found_new) = (false, false);
                let mut active_chat_lock = active_chat.lock().unwrap();
                let main_window: gtk::Grid =
                    unsafe { grid_base_grid.child_at(1, 0).unwrap().unsafe_cast() };
                let message_scrolled_window: gtk::ScrolledWindow =
                    unsafe { main_window.child_at(0, 0).unwrap().unsafe_cast() };
                let actual_chat_id = match chat_id {
                    Some(chat_id) => chat_id,
                    None => {
                        found_new = true;
                        -1
                    }
                };
                let active_chat_value = match *active_chat_lock {
                    Some(chat_id) => chat_id,
                    None => {
                        found_previous = true;
                        -1
                    }
                };
                if chat_id != *active_chat_lock {
                    for dialog in dialog_element_list_lock.iter() {
                        let data = unsafe {
                            dialog
                                .data::<grammers_client::types::Dialog>("dialog")
                                .unwrap()
                                .as_mut()
                        };
                        if !found_new && data.chat().id() == actual_chat_id {
                            found_new = true;
                            dialog.child().unwrap().add_css_class("dialog_active");
                            let message_view: gtk::ListBox = unsafe {
                                message_scrolled_window
                                    .first_child()
                                    .unwrap()
                                    .first_child()
                                    .unwrap()
                                    .unsafe_cast()
                            };
                            let mut messages_data = unsafe {
                                dialog
                                    .steal_data::<Vec<grammers_client::types::Message>>("messages")
                                    .unwrap()
                            };
                            message_view.remove_all();
                            if messages_data.len() == 0 {
                                let mut messages_iter = interface_handle
                                    .clone()
                                    .unwrap()
                                    .iter_messages(data.chat().pack())
                                    .limit(100);
                                futures::executor::block_on(async {
                                    while let Some(message) = messages_iter.next().await.unwrap() {
                                        messages_data.insert(0, message)
                                    }
                                });
                            }
                            for message in messages_data.clone() {
                                let (
                                    downloading_handle_clone,
                                    downloading_semaphore_clone,
                                    client_handle_clone,
                                ) = match message.photo() {
                                    Some(_) => (
                                        Some(downloading_handle.clone()),
                                        Some(downloading_semaphore.clone()),
                                        interface_handle.clone(),
                                    ),
                                    None => (None, None, None),
                                };
                                let message_row = create_message_row(
                                    &message,
                                    downloading_handle_clone,
                                    downloading_semaphore_clone,
                                    client_handle_clone,
                                    pool.clone(),
                                );
                                message_view.append(&message_row);
                            }
                            unsafe {
                                dialog.set_data("messages", messages_data);
                            }
                        } else if !found_previous && data.chat().id() == active_chat_value {
                            found_previous = true;
                            dialog.child().unwrap().remove_css_class("dialog_active");
                        }
                        if found_previous && found_new {
                            break;
                        }
                    }
                    *active_chat_lock = chat_id;
                }
                let overlay = main_window.child_at(0, 1).unwrap();
                match *active_chat_lock {
                    None => {
                        overlay.set_visible(false);
                    }
                    Some(_) => {
                        overlay.set_visible(true);
                        let textview: gtk::TextView = unsafe {
                            overlay
                                .first_child()
                                .unwrap()
                                .first_child()
                                .unwrap()
                                .unsafe_cast()
                        };
                        textview.grab_focus();
                        message_scrolled_window.queue_resize();
                        scroll_down(message_scrolled_window);
                    }
                }
            }
            InterfaceMessage::SendMessage => {
                let active_chat_lock = active_chat.lock().unwrap();
                let main_window: gtk::Grid =
                    unsafe { grid_base_grid.child_at(1, 0).unwrap().unsafe_cast() };
                let overlay = main_window.child_at(0, 1).unwrap();
                let textview: gtk::TextView = unsafe {
                    overlay
                        .first_child()
                        .unwrap()
                        .first_child()
                        .unwrap()
                        .unsafe_cast()
                };
                let buffer = textview.buffer();
                if *active_chat_lock != None {
                    let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), true);
                    let trimmed_text = text.trim();
                    if trimmed_text != "" {
                        let chat_id = active_chat_lock.unwrap();
                        let dialog_element_list_lock = dialog_element_list_mutex.lock().unwrap();
                        for dialog in dialog_element_list_lock.iter() {
                            let data = unsafe {
                                dialog
                                    .data::<grammers_client::types::Dialog>("dialog")
                                    .unwrap()
                                    .as_mut()
                            };
                            if data.chat().id() == chat_id {
                                let client_handle = interface_handle.clone().unwrap();
                                let _ = futures::executor::block_on(async {
                                    let message = client_handle
                                        .send_message(data.chat().pack(), trimmed_text)
                                        .await
                                        .unwrap();
                                    interface_sender.send(InterfaceMessage::NewMessage(message))
                                });
                                break;
                            }
                        }
                    }
                }
                buffer.set_text("");
            }
        }
        glib::ControlFlow::Continue
    });
    application.run();
}

fn scroll_down(scrolled_window: gtk::ScrolledWindow) {
    let vadjustment = scrolled_window.vadjustment();
    glib::idle_add_local(move || {
        // Scroll to the bottom of the window
        vadjustment.set_value(vadjustment.upper() - vadjustment.page_size());
        glib::ControlFlow::Break
    });
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

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../styles/main.css"));

    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(application: &gtk::Application, sender: glib::Sender<InterfaceMessage>) {
    let window = gtk::ApplicationWindow::builder()
        .application(application)
        .title("Crabgram")
        .default_width(640)
        .default_height(480)
        .build();

    let grid = gtk::Grid::new();

    let main_window = gtk::Grid::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(vec!["main_window"])
        .hexpand(true)
        .vexpand(true)
        .build();
    let message_view = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();
    let message_view_listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    message_view.set_child(Some(&message_view_listbox));
    let messsage_send_overlay = gtk::Overlay::builder()
        .css_classes(vec!["message_send_overlay"])
        .build();
    let message_send_box = gtk::TextView::builder()
        .css_classes(vec!["message_send_box"])
        .wrap_mode(gtk::WrapMode::WordChar)
        .hexpand(true)
        .build();
    let font_size = message_send_box
        .pango_context()
        .font_description()
        .unwrap()
        .size()
        / gtk::pango::SCALE;
    let message_send_scrollable = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&message_send_box)
        .propagate_natural_height(true)
        .max_content_height(font_size * 10)
        .build();
    let message_send_placeholder = gtk::Label::builder()
        .label("Write a message...")
        .css_classes(vec!["placeholder", "message_placeholder"])
        .halign(gtk::Align::Start)
        .can_target(false)
        .hexpand(true)
        .build();
    let message_send_placeholder_clone = message_send_placeholder.clone();
    let message_send_scrollable_clone = message_send_scrollable.clone();
    let message_send_key_controller = gtk::EventControllerKey::new();
    message_send_key_controller.connect_key_pressed(move |_, key, _, modifier| {
        if key == gtk::gdk::Key::Return {
            if !modifier.contains(gtk::gdk::ModifierType::SHIFT_MASK) {
                let _ = sender.send(InterfaceMessage::SendMessage);
                glib::Propagation::Stop
            } else {
                scroll_down(message_send_scrollable_clone.clone());
                glib::Propagation::Proceed
            }
        } else {
            glib::Propagation::Proceed
        }
    });
    message_send_box.add_controller(message_send_key_controller);
    message_send_box.buffer().connect_changed(move |buffer| {
        let value = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .to_string();
        if value == "" {
            message_send_placeholder_clone.set_visible(true);
        } else {
            message_send_placeholder_clone.set_visible(false);
        }
    });
    messsage_send_overlay.set_child(Some(&message_send_scrollable));
    messsage_send_overlay.add_overlay(&message_send_placeholder);
    messsage_send_overlay.set_visible(false);
    main_window.attach(&message_view, 0, 0, 1, 1);
    main_window.attach(&messsage_send_overlay, 0, 1, 1, 1);
    grid.attach(&main_window, 1, 0, 1, 1);

    let dialogs = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let dialogs_scroll_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Always)
        .child(&dialogs)
        .css_classes(vec!["dialogs"])
        .vexpand(true)
        .build();
    grid.attach(&dialogs_scroll_window, 0, 0, 1, 1);
    let dialog_listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    dialogs.append(&dialog_listbox);

    window.set_child(Some(&grid));

    window.present();
}

fn create_message_row(
    message: &grammers_client::types::Message,
    downloading_handle: Option<tokio::runtime::Handle>,
    downloading_semaphore: Option<Arc<tokio::sync::Semaphore>>,
    client_handle: Option<grammers_client::Client>,
    pool: sqlx::pool::Pool<sqlx::Sqlite>,
) -> gtk::ListBoxRow {
    let message_row = gtk::ListBoxRow::new();
    let constraint_layout = gtk::ConstraintLayout::new();
    let message_box = gtk::Box::builder()
        .css_classes(vec!["message"])
        .layout_manager(&constraint_layout)
        .hexpand(true)
        .build();
    message_row.set_child(Some(&message_box));
    let message_label = gtk::Label::builder()
        .css_classes(vec!["message_label"])
        .halign(gtk::Align::Start)
        .label(message.text())
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .build();
    let sender_label = gtk::Label::builder().halign(gtk::Align::Start).build();
    //let photo_box = gtk::DrawingArea::builder().visible(false).build();
    let photo_box = gtk::Picture::builder()
        .content_fit(gtk::ContentFit::Fill)
        .build();
    message_box.append(&photo_box);
    /*let height_constraint = gtk::Constraint::new(
        Some(&message_box),
        gtk::ConstraintAttribute::Height,
        gtk::ConstraintRelation::Eq,
        Some(&message_label),
        gtk::ConstraintAttribute::Height,
        1.0,
        110.0,
        1,
    );
    constraint_layout.add_constraint(height_constraint);*/
    let message_label_width_constraint = gtk::Constraint::new(
        Some(&message_label),
        gtk::ConstraintAttribute::Width,
        gtk::ConstraintRelation::Eq,
        Some(&message_box),
        gtk::ConstraintAttribute::Width,
        1.0,
        -10.0,
        1,
    );
    constraint_layout.add_constraint(message_label_width_constraint);
    let dumb_constraint = gtk::Constraint::new_constant(
        Some(&photo_box),
        gtk::ConstraintAttribute::Width,
        gtk::ConstraintRelation::Eq,
        100.0,
        1,
    );
    constraint_layout.add_constraint(dumb_constraint);
    let dumb_constraint2 = gtk::Constraint::new_constant(
        Some(&photo_box),
        gtk::ConstraintAttribute::Height,
        gtk::ConstraintRelation::Eq,
        100.0,
        1,
    );
    constraint_layout.add_constraint(dumb_constraint2);
    let sender_label_top_constraint = gtk::Constraint::new(
        Some(&sender_label),
        gtk::ConstraintAttribute::Top,
        gtk::ConstraintRelation::Eq,
        Some(&message_box),
        gtk::ConstraintAttribute::Top,
        1.0,
        1.0,
        1,
    );
    constraint_layout.add_constraint(sender_label_top_constraint);
    let photo_box_top_constraint = gtk::Constraint::new(
        Some(&photo_box),
        gtk::ConstraintAttribute::Top,
        gtk::ConstraintRelation::Eq,
        Some(&sender_label),
        gtk::ConstraintAttribute::Bottom,
        1.0,
        1.0,
        1,
    );
    constraint_layout.add_constraint(photo_box_top_constraint);
    let message_label_top_constraint = gtk::Constraint::new(
        Some(&message_label),
        gtk::ConstraintAttribute::Top,
        gtk::ConstraintRelation::Eq,
        Some(&photo_box),
        gtk::ConstraintAttribute::Bottom,
        1.0,
        1.0,
        1,
    );
    constraint_layout.add_constraint(message_label_top_constraint);
    if let Some(photo) = message.photo() {
        let downloading_handle = downloading_handle.unwrap();
        let downloading_semaphore = downloading_semaphore.unwrap();
        let client_clone = client_handle.unwrap();
        let (photo_sender, photo_receiver): (glib::Sender<String>, glib::Receiver<String>) =
            glib::MainContext::channel(glib::Priority::DEFAULT);
        photo_receiver.attach(None, move |photo_path| {
            photo_box.set_visible(true);
            photo_box.set_filename(Some(photo_path));
            glib::ControlFlow::Break
        });
        downloading_handle.spawn(async move {
            let mut conn = pool.acquire().await.unwrap();
            let photo_id = photo.id();
            let photo_path = format!("cache/{}", photo_id);
            let exists = sqlx::query!(r#"SELECT * FROM Photo WHERE photo_id = ?1"#, photo_id)
                .fetch_one(&mut *conn)
                .await;
            match exists {
                Err(_) => {
                    let _permit = downloading_semaphore.acquire().await.unwrap();
                    let downloadable = grammers_client::types::Downloadable::Media(
                        grammers_client::types::Media::Photo(photo.clone()),
                    );
                    let _ = client_clone
                        .download_media(&downloadable, photo_path.clone())
                        .await;
                    let _ = sqlx::query!(r#"INSERT INTO Photo(photo_id) VALUES(?1)"#, photo_id)
                        .execute(&mut *conn)
                        .await;
                }
                Ok(_) => {}
            }
            let _ = photo_sender.send(photo_path);
        });
    }
    if message.outgoing() {
        message_box.add_css_class("message_outgoing");
        sender_label.set_visible(false);
    } else {
        message_box.add_css_class("message_incoming");
    }
    message_box.append(&sender_label);
    match message.chat() {
        grammers_client::types::Chat::User(..) => {
            sender_label.set_visible(false);
        }
        _ => {
            sender_label.set_label(
                match message.sender() {
                    Some(sender) => sender.name().to_string(),
                    None => String::new(),
                }
                .as_ref(),
            );
        }
    }
    message_box.append(&message_label);
    message_row
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
*/
