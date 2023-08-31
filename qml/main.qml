import QtQuick 2.12
import QtQuick.Controls 2.12
import QtQuick.Window 2.12

// This must match the qml_uri and qml_version
// specified with the #[cxx_qt::qobject] macro in Rust.
import crabgram 1.0

Window {
    title: qsTr("Crabgram")
    visible: true
    height: 480
    width: 640
    color: "#0E1621"

    MyObject {
        id: myObject
        number: 0
    }

    Column {
        anchors.horizontalCenter: parent.horizontalRight
        anchors.verticalCenter: parent.verticalTop
        Label {
            id: label
            color: "#FFFFFF"
            text: "Label: " + myObject.number
        }

        Button {
            text: "Increment"
            onClicked: myObject.incrementNumber()
        }
    }
}
