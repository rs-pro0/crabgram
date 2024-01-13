slint::slint! {
    component Dialogs inherits VerticalLayout{
        Text{
            text: "Your dialogs:";
        }
        @children
    }
    component Dialog inherits Rectangle{
        in property <string> text;
        background: black;
        Text {
            text: text;
            color: red;
        }
    }
    export component MainWindow inherits Window {
        GridLayout{
            Dialogs{
                Dialog{text: "First";}
                Dialog{text: "Second";}
            }
            Text {
                text: "hello world";
                      color: green;
            }
        }
    }
}
