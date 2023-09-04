use glib::{clone, MainContext, Priority};
use gtk::glib;
use gtk::prelude::*;
use std::thread;

fn main() {
    let application = gtk::Application::builder()
        .application_id("crabgram")
        .build();
    application.connect_activate(build_ui);
    application.run();
}

enum MyMessage{
    NewMessage
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
    let (sender, receiver): (glib::Sender<MyMessage>, glib::Receiver<MyMessage>) = MainContext::channel(Priority::default());

    window.set_title(Some("Crabgram"));
    window.set_default_size(350, 70);

    let button = gtk::Button::with_label("Click me!");
    let dialogs = gtk::ColumnView::builder().build();
    let column = gtk::ColumnViewColumn::builder().build();
    let column_factory = column.factory().unwrap();
    column_factory.set
    dialogs.append_column(&column);
    let dialog = gtk::ColumnViewCell::builder().accessible_label("test").build();

    window.set_child(Some(&button));

    window.present();
}
