#include "session.h"
#include <QApplication>
#include <QBuffer>
#include <QClipboard>
#include <QDateTime>
#include <QMimeData>
#include <QSettings>

namespace kokoda {
Session::Session(QObject *parent) : QObject(parent) {
    preset = qBound(0, QSettings().value("quality", 2).toInt(), 3);
    core.action({{"type", "quality"}, {"preset", preset}});
    connect(QApplication::clipboard(), &QClipboard::dataChanged, this,
            [this] { ++clipboardVersion; });
    maintenance.setInterval(15);
    connect(&maintenance, &QTimer::timeout, this, &Session::tick);
    maintenance.start();
    retry.setSingleShot(true);
    connect(&retry, &QTimer::timeout, this, &Session::begin);
}
Session::~Session() {
    disconnectHost();
}
void Session::connectHost(QString host, QByteArray secret) {
    disconnectHost();
    address = host.trimmed();
    password = secret;
    wanted = true;
    retryCount = 0;
    begin();
}
void Session::begin() {
    if (!wanted)
        return;
    delete connection;
    connection = new Connection(this);
    auto epoch = ++generation;
    authenticated = false;
    streamStarted = false;
    queued.clear();
    pending.clear();
    connect(connection, &Connection::packet, this, [this, epoch](QJsonObject p) {
        if (epoch == generation)
            receive(p);
    });
    connect(connection, &Connection::lost, this, [this, epoch](QString category, bool retryable) {
        if (epoch == generation)
            lost(category, retryable);
    });
    emit status(text("status.connecting"));
    emit connection->start(address, password);
    // Wire sends the initial catalog itself; account for its FIFO response.
    pending.enqueue({{"command", "list_windows"}});
    emit changed();
}
void Session::disconnectHost() {
    wanted = false;
    retry.stop();
    ++generation;
    authenticated = false;
    streamStarted = false;
    queued.clear();
    pending.clear();
    copies.clear();
    active.clear();
    apps.clear();
    catalog = {};
    delete connection;
    connection = nullptr;
    password.fill('\0');
    password.clear();
    core.action({{"type", "disconnect"}});
    emit changed();
    emit status(text("status.disconnected"));
}
void Session::lost(QString category, bool retryable) {
    authenticated = false;
    streamStarted = false;
    queued.clear();
    pending.clear();
    copies.clear();
    active.clear();
    core.action({{"type", "disconnect"}});
    for (auto state : apps) {
        state->selected = false;
        state->qualityPending = true;
        for (auto *v : state->viewers)
            v->offline();
    }
    ++generation;
    catalog = {};
    if (connection) {
        connection->close();
        connection->deleteLater();
        connection = nullptr;
    }
    emit changed();
    if (wanted && autoReconnect && retryable && retryCount < 6) {
        ++retryCount;
        retry.start(qMin(10000, 500 * (1 << retryCount)));
        emit status(text("desktop.reconnecting"));
    } else {
        wanted = false;
        if (category == "authentication") {
            password.fill('\0');
            password.clear();
        }
        emit status(category == "authentication"
                        ? text("error.authentication")
                        : text("desktop.connection_failed") + " (" + category + ")");
    }
}
void Session::send(QJsonObject message) {
    const auto command = message["command"].toString();
    if (!authenticated && command != "list_windows")
        return;
    // Preserve clipboard/key boundaries; replace only adjacent cumulative pointer samples.
    if (command == "input" && !queued.isEmpty()) {
        auto &last = queued.last();
        auto events = message["events"].toArray();
        auto before = last["events"].toArray();
        if (last["command"] == "input" && last["id"] == message["id"] &&
            last["session_id"] == message["session_id"] && last["token"] == message["token"] &&
            before.size() < 128) {
            for (auto value : events) {
                auto next = value.toObject();
                if (!before.isEmpty()) {
                    auto old = before.last().toObject();
                    if ((next["kind"] == "move" || next["kind"] == "drag_move") &&
                        next["kind"] == old["kind"] && next["gesture"] == old["gesture"])
                        before.removeLast();
                }
                before.append(next);
            }
            last["events"] = before;
            return;
        }
    }
    if (command == "window_group" || command == "video_viewport" || command == "video_settings" ||
        command == "ping") {
        for (auto &old : queued)
            if (old["command"] == message["command"] &&
                old["session_id"] == message["session_id"] && old["id"] == message["id"]) {
                old = message;
                return;
            }
    }
    if (queued.size() >= 128) {
        lost("backlog", false);
        return;
    }
    queued.enqueue(message);
}
void Session::pump() {
    if (!connection)
        return;
    while (pending.size() < 16 && !queued.isEmpty()) {
        auto message = queued.dequeue();
        pending.enqueue(message);
        emit outgoing(message);
        emit connection->send(message);
    }
}
void Session::tick() {
    if (!authenticated)
        return;
    auto now = QDateTime::currentMSecsSinceEpoch();
    if (now - lastHeartbeat >= 1000) {
        lastHeartbeat = now;
        send({{"command", "ping"}});
    }
    if (streamStarted && now - lastGroup >= 300) {
        lastGroup = now;
        for (auto state : apps)
            if (state->selected)
                send({{"command", "window_group"}, {"session_id", qint64(state->sid)}});
    }
    if (syncClipboard && !readOnly && active && active->connected && active->canvas->allowed &&
        apps.contains(active->sid) && apps[active->sid]->selected && active->isActiveWindow() &&
        !active->dismissed && now - lastClipboard >= 500) {
        lastClipboard = now;
        auto request = ++nextClipboard;
        copies[request] = {active, clipboardVersion};
        send({{"command", "clipboard_poll"},
              {"session_id", qint64(active->sid)},
              {"id", qint64(active->id)},
              {"token", qint64(active->token)},
              {"request_id", qint64(request)},
              {"discard", clipboardBaseline}});
        clipboardBaseline = false;
    }
    pump();
}
void Session::reconnectHost() {
    if (!password.isEmpty()) {
        wanted = true;
        retryCount = 0;
        const bool automatic = autoReconnect;
        autoReconnect = true;
        lost("network", true);
        autoReconnect = automatic;
        retry.stop();
        begin();
    }
}
void Session::setAdaptiveResolution(bool enabled) {
    adaptiveResolution = enabled;
    for (auto state : apps)
        for (auto *v : state->viewers)
            v->setAdaptiveResolution(enabled);
}
void Session::refresh() {
    send({{"command", "list_windows"}});
    pump();
}
void Session::open(quint64 id) {
    if (!authenticated || !streamStarted)
        return;
    for (auto state : apps)
        if (state->root == id || state->viewers.contains(id)) {
            for (auto *v : state->viewers)
                v->showLocal();
            return;
        }
    bool found = false;
    for (auto v : catalog)
        if (number(v.toObject(), "id") == id)
            found = true;
    if (!found || apps.size() >= 8)
        return;
    auto state = std::make_shared<AppSession>();
    state->sid = ++nextSession;
    state->root = id;
    state->application = includeApplication;
    apps[state->sid] = state;
    auto p = core.profile(preset);
    p["command"] = "select_window";
    p["id"] = qint64(id);
    p["session_id"] = qint64(state->sid);
    p["scope"] = state->application ? "application" : "window";
    p["alpha_video"] = true;
    p["playback_feedback"] = true;
    p["window_management"] = true;
    p["resolution_bitrate"] = true;
    send(p);
    pump();
}
void Session::remove(quint64 sid) {
    if (active && active->sid == sid)
        active.clear();
    apps.remove(sid);
    core.action({{"type", "forget"}, {"session", qint64(sid)}});
    emit changed();
}
void Session::end(quint64 sid) {
    if (!apps.contains(sid))
        return;
    for (auto it = queued.begin(); it != queued.end();)
        if (number(*it, "session_id") == sid)
            it = queued.erase(it);
        else
            ++it;
    for (auto it = copies.begin(); it != copies.end();)
        if (!it->viewer || it->viewer->sid == sid)
            it = copies.erase(it);
        else
            ++it;
    send({{"command", "close_window"}, {"session_id", qint64(sid)}});
    remove(sid);
    pump();
}
void Session::setDisplayScale(int percent) {
    if (active)
        active->setDisplayScale(percent);
}
void Session::reopen() {
    for (auto state : apps)
        for (auto *v : state->viewers)
            v->showLocal();
}
void Session::setQuality(int value) {
    if (value < 0 || value > 3)
        return;
    preset = value;
    QSettings().setValue("quality", value);
    core.action({{"type", "quality"}, {"preset", value}});
    for (auto state : apps) {
        state->qualityPending = true;
        if (state->selected && !state->viewers.isEmpty()) {
            auto *root = state->viewers.value(state->root);
            if (!root)
                continue;
            auto p = core.profile(preset);
            p["command"] = "video_settings";
            p["session_id"] = qint64(state->sid);
            p["id"] = qint64(root->id);
            p["token"] = qint64(root->token);
            p["resolution_bitrate"] = true;
            send(p);
            state->qualityPending = false;
        }
    }
    pump();
}
void Session::group(AppSession &state, QJsonObject message) {
    auto members = message["windows"].toArray();
    int otherMembers = 0;
    for (auto app : apps)
        if (app.get() != &state)
            otherMembers += app->viewers.size();
    if (members.size() + otherMembers > 64) {
        lost("protocol", false);
        return;
    }
    state.root = number(message, "root_id");
    QSet<quint64> included;
    struct Preference {
        QRect geometry;
        bool dismissed;
        bool hostMode;
    };
    QMap<quint64, Preference> restored;
    for (auto item : members) {
        auto entry = item.toObject();
        auto id = number(entry, "id"), token = number(entry, "token");
        if (!id || !token)
            continue;
        included.insert(id);
        auto *v = state.viewers.value(id);
        if (v && v->token != token) {
            const auto identity = QString::number(number(entry, "process_id")) + ":" +
                                  QString::number(number(entry, "thread_id"));
            if (!v->connected && v->identity == identity)
                restored[id] = {v->geometry(), v->dismissed, v->hostMode};
            state.viewers.remove(id);
            delete v;
            v = nullptr;
        }
        if (!v) {
            v = new Viewer(state.sid, id, token);
            v->setAdaptiveResolution(adaptiveResolution);
            if (restored.contains(id))
                v->dismissed = restored[id].dismissed;
            state.viewers[id] = v;
            connect(v, &Viewer::request, this, [this, v](QJsonObject p) {
                if (!authenticated || !apps.contains(v->sid) ||
                    apps[v->sid]->viewers.value(v->id) != v || !apps[v->sid]->selected ||
                    !v->connected)
                    return;
                p["session_id"] = qint64(v->sid);
                p["id"] = qint64(v->id);
                p["token"] = qint64(v->token);
                send(p);
            });
            connect(v, &Viewer::focused, this, [this, v] {
                if (!v->connected || !v->canvas->allowed || !apps.contains(v->sid) ||
                    !apps[v->sid]->selected)
                    return;
                if (active != v) {
                    if (active)
                        active->canvas->releaseInput();
                    active = v;
                    clipboardBaseline = true;
                }
            });
            connect(v, &Viewer::clipboard, this,
                    [this, v](QString action) { clipboard(v, action); });
            connect(
                v, &Viewer::failed, this,
                [this, sid = state.sid](QString reason) {
                    emit status(reason);
                    end(sid);
                },
                Qt::QueuedConnection);
            connect(v, &Viewer::localClosed, this, [this, sid = state.sid](quint64 parent) {
                if (!apps.contains(sid))
                    return;
                QSet<quint64> hidden{parent};
                bool changed = true;
                while (changed) {
                    changed = false;
                    for (auto *child : apps[sid]->viewers)
                        if (child->role != "independent" && hidden.contains(child->owner) &&
                            !hidden.contains(child->id)) {
                            child->dismiss();
                            hidden.insert(child->id);
                            changed = true;
                        }
                }
            });
        }
    }
    for (auto id : state.viewers.keys())
        if (!included.contains(id))
            delete state.viewers.take(id);
    // Native owners are resolved only among authorized session members.
    for (auto item : members) {
        auto entry = item.toObject();
        auto *v = state.viewers.value(number(entry, "id"));
        if (v) {
            v->connected = true;
            v->updateWindow(entry, readOnly,
                            state.viewers.value(number(entry["presentation"].toObject(), "owner")));
            if (restored.contains(v->id)) {
                auto preference = restored[v->id];
                v->setGeometry(preference.geometry);
                v->setMode(preference.hostMode);
            }
        }
    }
    if (auto *root = state.viewers.value(state.root))
        core.action({{"type", "track"},
                     {"session", qint64(state.sid)},
                     {"id", qint64(root->id)},
                     {"generation", qint64(root->token)}});
    if (state.qualityPending)
        setQuality(preset);
    emit changed();
}
void Session::receive(QJsonObject message) {
    QJsonObject request;
    if (!message["event"].toBool() && !pending.isEmpty())
        request = pending.dequeue();
    auto type = message["type"].toString();
    auto sid = number(message, "session_id");
    auto state = apps.value(sid);
    if (type == "windows") {
        bool first = !authenticated;
        authenticated = true;
        readOnly = message["read_only"].toBool(true);
        catalog = message["windows"].toArray();
        if (!message["window_management"].toBool() || !message["playback_feedback"].toBool() ||
            !message["push_video"].toBool()) {
            lost("host_upgrade", false);
            return;
        }
        if (first)
            send({{"command", "start_stream"}});
        emit status(text(readOnly ? "sharing.read_only" : "status.choose_window"));
        emit changed();
    } else if (type == "stream_started") {
        streamStarted = true;
        retryCount = 0;
        for (auto app : apps)
            if (!app->ticket.isEmpty())
                send({{"command", "resume_window"},
                      {"session_id", qint64(app->sid)},
                      {"resume_token", app->ticket}});
        emit changed();
    } else if (type == "selected" && state) {
        state->selected = true;
        state->root = number(message, "id");
        state->ticket = message["resume_token"].toString();
        send({{"command", "window_group"}, {"session_id", qint64(sid)}});
    } else if (type == "window_group" && state)
        group(*state, message);
    else if (type == "frame" && state) {
        auto *v = state->viewers.value(number(message, "id"));
        if (v && v->token == number(message, "token") && !v->frame(message))
            lost("backlog", false);
    } else if (type == "window_action_ok" && state) {
        if (auto *v = state->viewers.value(number(message, "id")))
            v->resizeAck(message);
    } else if (type == "clipboard_result") {
        auto copy = copies.take(number(message, "request_id"));
        if (!copy.viewer || copy.viewer != active || !active->isActiveWindow() ||
            clipboardVersion != copy.version || message["action"] == "paste" ||
            message["changed"] == false) {
            pump();
            return;
        }
        if (message["image_png"].isString()) {
            auto image =
                checkedImage(QByteArray::fromBase64(message["image_png"].toString().toLatin1()));
            if (!image.isNull())
                QApplication::clipboard()->setImage(image);
        } else if (message["text"].isString())
            QApplication::clipboard()->setText(message["text"].toString());
    } else if (type == "closed")
        remove(sid);
    else if (type == "error") {
        copies.remove(number(message, "request_id"));
        emit status(text("error.remote") + " " + message["message"].toString());
        if (message["selection_lost"].toBool() || request["command"] == "select_window" ||
            request["command"] == "resume_window")
            remove(sid);
    }
    pump();
}
void Session::clipboard(Viewer *v, QString action) {
    if (!authenticated || readOnly || !v->connected || !v->canvas->allowed ||
        !apps.contains(v->sid) || !apps[v->sid]->selected)
        return;
    QJsonObject p{{"command", "clipboard"},
                  {"session_id", qint64(v->sid)},
                  {"id", qint64(v->id)},
                  {"token", qint64(v->token)},
                  {"action", action}};
    if (action == "paste") {
        auto *mime = QApplication::clipboard()->mimeData();
        if (mime->hasUrls()) {
            emit status(text("clipboard.files_unsupported"));
            return;
        }
        if (mime->hasImage()) {
            auto image = QApplication::clipboard()->image();
            if (qint64(image.width()) * image.height() > 16777216) {
                emit status(text("clipboard.image_limit"));
                return;
            }
            QByteArray png;
            QBuffer buffer(&png);
            buffer.open(QIODevice::WriteOnly);
            if (!image.save(&buffer, "PNG") || png.size() > 8 * 1024 * 1024) {
                emit status(text("clipboard.image_limit"));
                return;
            }
            p["image_png"] = QString::fromLatin1(png.toBase64());
        } else if (mime->hasText()) {
            auto value = mime->text();
            if (value.toUtf8().size() > 1048576 || value.contains(QChar(0))) {
                emit status(text("clipboard.text_limit"));
                return;
            }
            p["text"] = value;
        } else {
            emit status(text("clipboard.empty"));
            return;
        }
    }
    auto request = ++nextClipboard;
    p["request_id"] = qint64(request);
    copies[request] = {v, clipboardVersion};
    send(p);
    pump();
}
} // namespace kokoda
