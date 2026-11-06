import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtMultimedia

// A video from the thread's folder (docs: Design 09): the picture, a play button over it, and
// play / time / seek underneath. It streams from the host's file link, so it can seek.
Item {
    id: video
    property alias source: player.source

    MediaPlayer {
        id: player
        videoOutput: out
        audioOutput: AudioOutput {}
    }
    function clock(ms) {
        const s = Math.floor(ms / 1000)
        return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0")
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: width * 9 / 16
            radius: 8
            color: theme.well
            clip: true
            VideoOutput { id: out; anchors.fill: parent; fillMode: VideoOutput.PreserveAspectFit }
            Rectangle {
                visible: player.playbackState !== MediaPlayer.PlayingState
                anchors.centerIn: parent
                width: 48; height: 48; radius: 24
                color: theme.text
                Icon { anchors.centerIn: parent; anchors.horizontalCenterOffset: 2; path: "M8 5.5v13l11-6.5z"; filled: true; size: 20; color: theme.onLight }
            }
            Text {
                visible: player.error !== MediaPlayer.NoError
                anchors.bottom: parent.bottom; anchors.horizontalCenter: parent.horizontalCenter; anchors.bottomMargin: 10
                text: "Can't play this video: " + player.errorString
                color: theme.red; font.pixelSize: theme.small
            }
            MouseArea { anchors.fill: parent; onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play() }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Btn {
                kind: "icon"; small: true
                icon: player.playbackState === MediaPlayer.PlayingState ? "pause" : ""
                onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play()
                Icon { visible: player.playbackState !== MediaPlayer.PlayingState; anchors.centerIn: parent; path: "M8 5.5v13l11-6.5z"; filled: true; size: 12; color: theme.text }
            }
            Text { text: video.clock(player.position) + " / " + video.clock(player.duration); color: theme.text2; font.pixelSize: theme.tiny }
            Slider {
                id: seek
                Layout.fillWidth: true
                from: 0; to: Math.max(1, player.duration)
                value: player.position
                onMoved: player.position = value
                background: Rectangle {
                    x: seek.leftPadding; y: seek.topPadding + seek.availableHeight / 2 - 1.5
                    width: seek.availableWidth; height: 3; radius: 2; color: theme.selection
                    Rectangle { width: seek.visualPosition * parent.width; height: 3; radius: 2; color: theme.text2 }
                }
                handle: Rectangle {
                    x: seek.leftPadding + seek.visualPosition * (seek.availableWidth - width)
                    y: seek.topPadding + seek.availableHeight / 2 - height / 2
                    width: 3; height: 14; radius: 1; color: theme.blue
                }
            }
        }
        Item { Layout.fillHeight: true }
    }
}
