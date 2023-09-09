use glib::{clone, MainContext, Priority};
use gtk::glib;
use gtk::prelude::*;
use std::thread;

fn main() {
    let application = gtk::Application::builder()
        .application_id("crabgram")
        .build();
    application.connect_startup(|_| load_css());
    application.connect_activate(build_ui);
    thread::spawn(|| {
        
    });
    application.run();
}

enum MyMessage {
    NewMessage,
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
    let (sender, receiver): (glib::Sender<MyMessage>, glib::Receiver<MyMessage>) =
        MainContext::channel(Priority::default());

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

    // Create buttons and add them to the ListBox.
    for i in 1..6 {
        let button_label = format!("Button {}", i);
        let button = gtk::Button::with_label(&button_label);

        // You can connect signals or add actions to the buttons here if needed.

        let row = gtk::ListBoxRow::new();
        let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row_box.add_css_class("dialog");
        row_box.append(&button);
        row.set_child(Some(&row_box));
        listbox.append(&row);
    }

    // Set the ListBox as the content of the window.
    window.set_child(Some(&grid));

    window.present();
}
