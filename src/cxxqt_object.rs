#[cxx_qt::bridge]
mod my_object {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        include!("cxx-qt-lib/qvector.h");
        type QString = cxx_qt_lib::QString;
        type QVector_QString = cxx_qt_lib::QVector<QString>;
    }

    #[cxx_qt::qobject(qml_uri = "crabgram", qml_version = "1.0")]
    pub struct MyObject {
        #[qproperty]
        number: i32,
        #[qproperty]
        string: QString,
        #[qproperty]
        dialogs: Vec<QString>,
    }

    impl Default for MyObject {
        fn default() -> Self {
            Self {
                number: 0,
                string: QString::from(""),
                dialogs: Vec::new(),
            }
        }
    }

    impl qobject::MyObject {
        /*#[qinvokable]
        pub fn increment_number(self: Pin<&mut Self>) {
            let previous = *self.as_ref().number();
            self.set_number(previous + 1);
        }*/
        #[qinvokable]
        pub fn increment_number(self: Pin<&mut Self>) {
            let previous = *self.as_ref().number();
            self.set_number(previous + 1);
        }
    }
}
