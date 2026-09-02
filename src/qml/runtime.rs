use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

pub(crate) fn write_runtime(output_root: &Path) -> Result<()> {
    let directory = output_root.join(".vue2qml");
    fs::create_dir_all(&directory).map_err(|error| Error::io(error, &directory))?;
    write(&directory.join("qmldir"), QMLDIR)?;
    write(&directory.join("Runtime.qml"), RUNTIME_QML)?;
    write(&directory.join("VueElement.qml"), VUE_ELEMENT_QML)?;
    Ok(())
}

fn write(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content).map_err(|error| Error::io(error, path))
}

const QMLDIR: &str = "singleton Runtime 1.0 Runtime.qml\nVueElement 1.0 VueElement.qml\n";

const RUNTIME_QML: &str = r#"pragma Singleton
import QtQml

QtObject {
    function display(value) {
        if (value === null || value === undefined) {
            return ""
        }
        return String(value)
    }

    function read(object, name) {
        if (object === null || object === undefined) {
            return null
        }
        const value = object[name]
        if (value && typeof value === "object" && "value" in value) {
            return value.value
        }
        return value === undefined ? null : value
    }

    function method(object, name) {
        return function() {
            if (object === null || object === undefined) {
                return null
            }
            let candidate = object[name]
            if (candidate && typeof candidate === "object" && "value" in candidate) {
                candidate = candidate.value
            }
            return typeof candidate === "function" ? candidate.apply(object, arguments) : null
        }
    }

    function toModel(value) {
        if (value === null || value === undefined || value === false) {
            return []
        }
        return value
    }

    function sourceExpression(source) {
        return null
    }

    function prepareEvent(event, modifiers) {
        if (modifiers.indexOf("self") !== -1 && event && event.target !== event.currentTarget) {
            return false
        }
        if (modifiers.indexOf("enter") !== -1 && event && event.key !== "Enter") {
            return false
        }
        if (modifiers.indexOf("space") !== -1 && event && event.key !== " ") {
            return false
        }
        if (event && (modifiers.indexOf("prevent") !== -1 || modifiers.indexOf("stop") !== -1)) {
            event.accepted = true
        }
        return true
    }
}
"#;

const VUE_ELEMENT_QML: &str = r#"import QtQuick

Item {
    id: root

    property string tag: "div"
    property string componentName: ""
    property string sourcePath: ""
    property var requiredProperties: []
    property var staticAttributes: ({})
    property var dynamicAttributes: ({})
    property var directives: ({})
    property var styleSheets: []
    property string textContent: ""
    property var htmlContent: null
    property var modelValue: null
    property var vueKey: null
    property string vueRef: ""
    property string slotName: ""
    property bool condition: true
    property color textColor: "white"

    visible: root.condition
    implicitWidth: Math.max(1, root.childrenRect.width)
    implicitHeight: Math.max(root.textContent.length > 0 ? vueText.implicitHeight : 1,
                             root.childrenRect.height)

    signal vueClicked(var event)
    signal vueKeydown(var event)
    signal vueKeyup(var event)
    signal vueContextmenu(var event)
    signal vueChanged(var event)
    signal vueInput(var event)
    signal vueSubmitted(var event)
    signal vueError(var event)
    signal vueLoaded(var event)
    signal vueLoadeddata(var event)
    signal vueLoadedmetadata(var event)
    signal vueCanplay(var event)
    signal vuePlayed(var event)
    signal vuePlaying(var event)
    signal vuePaused(var event)
    signal vueWaiting(var event)
    signal vueStalled(var event)
    signal vueEnded(var event)
    signal vueTimeupdate(var event)
    signal vueMousedown(var event)
    signal vueMouseenter(var event)
    signal vueWheel(var event)
    signal vueScroll(var event)
    signal vueTouchstart(var event)
    signal vuePointerdown(var event)
    signal vuePointermove(var event)
    signal vuePointerup(var event)
    signal vuePointercancel(var event)
    signal vueDragstart(var event)
    signal vueDragend(var event)
    signal vueDragover(var event)
    signal vueDropped(var event)
    signal vuePasted(var event)
    signal vuePanned(var event)
    signal vueUpdateModelValue(var event)

    Text {
        id: vueText
        visible: root.textContent.length > 0
        text: root.textContent
        color: root.textColor
        wrapMode: Text.Wrap
    }

    TapHandler {
        onTapped: root.vueClicked(null)
    }
}
"#;
