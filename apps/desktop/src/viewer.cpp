#include "viewer.h"
#include "integration.h"
#include <QApplication>
#include <QCloseEvent>
#include <QDateTime>
#include <QGuiApplication>
#include <QHBoxLayout>
#include <QInputMethod>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QLabel>
#include <QMenu>
#include <QMouseEvent>
#include <QPainter>
#include <QResizeEvent>
#include <QScreen>
#include <QSignalBlocker>
#include <QStandardItemModel>
#include <QVBoxLayout>
#include <QWindow>
#include <chrono>

namespace kokoda {
static int virtualKey(QKeyEvent *e) {
    if (e->key() >= Qt::Key_A && e->key() <= Qt::Key_Z)
        return e->key();
    if (e->key() >= Qt::Key_0 && e->key() <= Qt::Key_9)
        return e->key();
    if (e->key() >= Qt::Key_F1 && e->key() <= Qt::Key_F24)
        return 0x70 + e->key() - Qt::Key_F1;
    switch (e->key()) {
    case Qt::Key_Backspace:
        return 8;
    case Qt::Key_Tab:
    case Qt::Key_Backtab:
        return 9;
    case Qt::Key_Return:
    case Qt::Key_Enter:
        return 13;
    case Qt::Key_Shift:
        return 16;
    case Qt::Key_Control:
        return 17;
    case Qt::Key_Alt:
        return 18;
    case Qt::Key_Escape:
        return 27;
    case Qt::Key_Space:
        return 32;
    case Qt::Key_PageUp:
        return 33;
    case Qt::Key_PageDown:
        return 34;
    case Qt::Key_End:
        return 35;
    case Qt::Key_Home:
        return 36;
    case Qt::Key_Left:
        return 37;
    case Qt::Key_Up:
        return 38;
    case Qt::Key_Right:
        return 39;
    case Qt::Key_Down:
        return 40;
    case Qt::Key_Insert:
        return 45;
    case Qt::Key_Delete:
        return 46;
    default:
        return 0;
    }
}
Canvas::Canvas(QWidget *parent) : QWidget(parent) {
    setFocusPolicy(Qt::StrongFocus);
    setAttribute(Qt::WA_InputMethodEnabled);
    setMouseTracking(true);
}
QRectF Canvas::imageRect() const {
    if (source.isEmpty())
        return rect();
    QSizeF size = source;
    size.scale(this->size(), Qt::KeepAspectRatio);
    if (width() - size.width() <= 1.01 && height() - size.height() <= 1.01) {
        size = source;
        size.scale(this->size(), Qt::KeepAspectRatioByExpanding);
    }
    return {(width() - size.width()) / 2, (height() - size.height()) / 2, size.width(),
            size.height()};
}
bool Canvas::event(QEvent *event) {
    if (allowed && event->type() == QEvent::ShortcutOverride) {
        event->accept();
        return true;
    }
    if (allowed && (event->type() == QEvent::KeyPress || event->type() == QEvent::KeyRelease)) {
        auto *key = static_cast<QKeyEvent *>(event);
        if (key->key() == Qt::Key_Tab || key->key() == Qt::Key_Backtab) {
            if (event->type() == QEvent::KeyPress)
                keyPressEvent(key);
            else
                keyReleaseEvent(key);
            return true;
        }
    }
    return QWidget::event(event);
}
void Canvas::paintEvent(QPaintEvent *) {
    QPainter p(this);
    p.fillRect(rect(), palette().window());
    p.setRenderHint(QPainter::SmoothPixmapTransform);
    if (!image.isNull())
        p.drawImage(imageRect(), image);
    if (!preedit.isEmpty()) {
        auto display = imageRect();
        QRectF r(display.x() + caret.x() * display.width(),
                 display.y() + caret.y() * display.height(),
                 QFontMetrics(font()).horizontalAdvance(preedit) + 12, fontMetrics().height() + 6);
        p.fillRect(r, palette().base());
        p.setPen(palette().text().color());
        p.drawText(r.adjusted(4, 0, -4, 0), Qt::AlignVCenter, preedit);
    }
}
QVariant Canvas::inputMethodQuery(Qt::InputMethodQuery query) const {
    if (query == Qt::ImEnabled)
        return allowed;
    if (query == Qt::ImCursorRectangle) {
        auto display = imageRect();
        return QRectF(display.x() + caret.x() * display.width(),
                      display.y() + caret.y() * display.height(), 1, fontMetrics().height());
    }
    if (query == Qt::ImCursorPosition || query == Qt::ImAnchorPosition)
        return 0;
    if (query == Qt::ImSurroundingText || query == Qt::ImCurrentSelection)
        return QString();
    if (query == Qt::ImHints)
        return int(Qt::ImhNoPredictiveText);
    return QWidget::inputMethodQuery(query);
}
void Canvas::inputMethodEvent(QInputMethodEvent *event) {
    if (!allowed) {
        event->ignore();
        return;
    }
    preedit = event->preeditString();
    if (!event->commitString().isEmpty())
        emit input({{"kind", "text"}, {"text", event->commitString()}});
    event->accept();
    update();
}
void Canvas::releaseInput() {
    if (gesture || !pressed.isEmpty() || !buttons.isEmpty())
        emit input({{"kind", "release"}});
    gesture = 0;
    pressed.clear();
    buttons.clear();
    preedit.clear();
    if (hasFocus())
        QGuiApplication::inputMethod()->reset();
    update();
}
void Canvas::keyPressEvent(QKeyEvent *event) {
    if (!allowed)
        return;
    if (event->matches(QKeySequence::Copy) || event->matches(QKeySequence::Cut) ||
        event->matches(QKeySequence::Paste)) {
        QGuiApplication::inputMethod()->reset();
        preedit.clear();
        emit input({{"kind", "release"}});
        pressed.clear();
        emit clipboard(event->matches(QKeySequence::Copy)  ? "copy"
                       : event->matches(QKeySequence::Cut) ? "cut"
                                                           : "paste");
        return;
    }
    if (!preedit.isEmpty()) {
        event->ignore();
        return;
    }
    const bool shortcut = event->modifiers().testFlag(Qt::ControlModifier) ||
                          event->modifiers().testFlag(Qt::AltModifier) ||
                          event->modifiers().testFlag(Qt::MetaModifier);
    if (!shortcut && !event->text().isEmpty() && event->text()[0].isPrint())
        emit input({{"kind", "text"}, {"text", event->text()}});
    else if (int vk = virtualKey(event)) {
        pressed.insert(vk);
        emit input({{"kind", "key"}, {"vk", vk}, {"down", true}});
    }
}
void Canvas::keyReleaseEvent(QKeyEvent *event) {
    if (event->isAutoRepeat())
        return;
    int vk = virtualKey(event);
    if (pressed.remove(vk))
        emit input({{"kind", "key"}, {"vk", vk}, {"down", false}});
}
QJsonObject Canvas::point(QPointF p, const char *kind) const {
    auto display = imageRect();
    return {{"kind", kind},
            {"x", qBound(0., (p.x() - display.x()) / qMax(1., display.width()), 0.999999)},
            {"y", qBound(0., (p.y() - display.y()) / qMax(1., display.height()), 0.999999)},
            {"width", source.width()},
            {"height", source.height()}};
}
QJsonObject Canvas::delta(QPointF p, const char *kind) const {
    QPointF d = p - dragOrigin;
    return {{"kind", kind},
            {"gesture", qint64(gesture)},
            {"dx", d.x() * dragPixels.width() / qMax(1, dragView.width())},
            {"dy", d.y() * dragPixels.height() / qMax(1, dragView.height())}};
}
void Canvas::mousePressEvent(QMouseEvent *e) {
    if (!allowed || !imageRect().contains(e->position()))
        return;
    setFocus(Qt::MouseFocusReason);
    emit focused();
    auto display = imageRect();
    caret = {(e->position().x() - display.x()) / display.width(),
             (e->position().y() - display.y()) / display.height()};
    QGuiApplication::inputMethod()->update(Qt::ImCursorRectangle);
    buttons.insert(int(e->button()));
    if (e->button() == Qt::LeftButton && anchored) {
        static quint64 serial = 0;
        gesture = ++serial;
        dragOrigin = e->globalPosition();
        dragPixels = source;
        dragView = imageRect().size().toSize();
        auto event = point(e->position(), "drag_begin");
        event["gesture"] = qint64(gesture);
        emit input(event);
    } else {
        auto event = point(e->position(), "button");
        event["button"] = e->button() == Qt::LeftButton    ? 0
                          : e->button() == Qt::RightButton ? 1
                                                           : 2;
        event["down"] = true;
        emit input(event);
    }
}
void Canvas::mouseMoveEvent(QMouseEvent *e) {
    if (!allowed || (!hasFocus() && !gesture))
        return;
    emit input(gesture ? delta(e->globalPosition(), "drag_move") : point(e->position(), "move"));
}
void Canvas::mouseReleaseEvent(QMouseEvent *e) {
    if (!allowed)
        return;
    buttons.remove(int(e->button()));
    if (e->button() == Qt::LeftButton && gesture) {
        emit input(delta(e->globalPosition(), "drag_end"));
        gesture = 0;
    } else {
        auto event = point(e->position(), "button");
        event["button"] = e->button() == Qt::LeftButton    ? 0
                          : e->button() == Qt::RightButton ? 1
                                                           : 2;
        event["down"] = false;
        emit input(event);
    }
}
void Canvas::wheelEvent(QWheelEvent *e) {
    if (!allowed || gesture || source.isEmpty() || !imageRect().contains(e->position())) return;
    const int delta=qBound(-1200,e->angleDelta().y(),1200);
    if (!delta) return;
    setFocus();
    auto event=point(e->position(),positionedWheel ? "wheel_at":"move");
    if (positionedWheel) {event["delta"]=delta;emit input(event);}
    else {emit input(event);emit input({{"kind","wheel"},{"delta",delta}});}
    e->accept();
}
void Canvas::focusInEvent(QFocusEvent *e) {
    QWidget::focusInEvent(e);
    emit focused();
    QGuiApplication::inputMethod()->update(Qt::ImQueryAll);
}
void Canvas::focusOutEvent(QFocusEvent *e) {
    releaseInput();
    QWidget::focusOutEvent(e);
}

Viewer::Viewer(quint64 session, quint64 window, quint64 generation)
    : sid(session), id(window), token(generation) {
    setAttribute(Qt::WA_DeleteOnClose, false);
    setWindowTitle("Kokoda");
    auto *layout = new QVBoxLayout(this);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    bar = new QWidget(this);
    auto *row = new QHBoxLayout(bar);
    row->setContentsMargins(6, 2, 6, 2);
    mode = new QComboBox(bar);
    mode->addItems({text("viewer.scale_local"), text("viewer.resize_host")});
    mode->setAccessibleName(text("viewer.resize_mode"));
    statistics = new QLabel;
    statistics->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
    row->addWidget(statistics, 1);
    bar->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(bar, &QWidget::customContextMenuRequested, this, [this](QPoint point) {
        QMenu menu;
        auto *zoom = menu.addMenu(text("menu.viewer_zoom"));
        for (int percent : {50, 75, 100, 125, 150, 200}) {
            auto *action = zoom->addAction(QString::number(percent) + "%", this,
                                           [this, percent] { setDisplayScale(percent); });
            action->setEnabled(!isMaximized() && !isMinimized());
        }
        menu.exec(bar->mapToGlobal(point));
    });
    row->addWidget(mode);
    layout->addWidget(bar);
    canvas = new Canvas(this);
    layout->addWidget(canvas, 1);
    player = new Player(this);
    connect(mode, qOverload<int>(&QComboBox::activated), this, [this](int n) {
        setMode(n == 1);
        canvas->setFocus();
    });
    connect(canvas, &Canvas::input, this, [this](QJsonObject e) {
        if (connected && (!readOnly || e["kind"] == "release"))
            emit request({{"command", "input"}, {"events", QJsonArray{e}}});
    });
    connect(canvas, &Canvas::clipboard, this, &Viewer::clipboard);
    connect(canvas, &Canvas::focused, this, &Viewer::focused);
    settle.setSingleShot(true);
    settle.setInterval(120);
    connect(&settle, &QTimer::timeout, this, [this] {
        if (!geometryPending || !hostMode || !connected)
            return;
        geometryPending = false;
        const auto size = canvas->size();
        const auto w = qBound(
            64, qRound(initialHost.width() * size.width() / qMax(1, initialContent.width())), 8192);
        const auto h = qBound(
            64, qRound(initialHost.height() * size.height() / qMax(1, initialContent.height())),
            8192);
        ignoreRemoteUntil = QDateTime::currentMSecsSinceEpoch() + 750;
        emit request({{"command", "window_action"},
                      {"action", "resize"},
                      {"width", w},
                      {"height", h},
                      {"revision", qint64(++revision)},
                      {"final", true}});
    });
    viewport.setSingleShot(true);
    viewport.setInterval(250);
    connect(&viewport, &QTimer::timeout, this, [this] {
        if (connected && !dismissed && !canvas->source.isEmpty())
            emit request(
                {{"command", "video_viewport"},
                 {"width",
                  adaptive ? qMin(8192, qRound(canvas->imageRect().width() * devicePixelRatioF()))
                           : 0},
                 {"height",
                  adaptive ? qMin(8192, qRound(canvas->imageRect().height() * devicePixelRatioF()))
                           : 0}});
    });
    connect(player, &Player::decoded, this,
            [this](QJsonObject p, QImage image, double ms, QString error) {
                if (!connected || number(p, "_local_generation") != localGeneration)
                    return;
                if (!error.isEmpty()) {
                    emit failed(text("desktop.decode_failed"));
                    return;
                }
                auto old = canvas->source;
                canvas->source = {p["width"].toInt(), p["height"].toInt()};
                if (!image.isNull())
                    canvas->image = image;
                auto caret = p["caret"].toArray();
                if (caret.size() == 2) {
                    canvas->caret = {caret[0].toDouble(), caret[1].toDouble()};
                    QGuiApplication::inputMethod()->update(Qt::ImCursorRectangle);
                }
                if (old != canvas->source && !hostMode)
                    fit();
                canvas->update();
                auto now = QDateTime::currentMSecsSinceEpoch();
                if (!dismissed && !isMinimized() && !image.isNull())
                    ++presented;
                if (statsAt && now - statsAt >= 1000) {
                    statistics->setText(text("desktop.frame_stats")
                                            .arg(presented * 1000.0 / (now - statsAt), 0, 'f', 1)
                                            .arg(image.width())
                                            .arg(image.height()));
                    presented = 0;
                    statsAt = now;
                } else if (!statsAt)
                    statsAt = now;
                feedback(p, ms);
            });
}
Viewer::~Viewer() {
    // Windows destroys native owned HWNDs with their owner. Detach surviving
    // viewers before destroying this HWND so Qt retains their actual lifetime.
    for (auto *widget : QApplication::topLevelWidgets()) {
        auto *child = qobject_cast<Viewer *>(widget);
        if (child && child != this && child->windowHandle() &&
            child->windowHandle()->transientParent() == windowHandle())
            setNativeOwner(child, nullptr, child->role == "independent");
    }
}
void Viewer::setAdaptiveResolution(bool enabled) {
    adaptive = enabled;
    viewport.start();
}
void Viewer::setDisplayScale(int percent) {
    if (percent < 50 || percent > 200 || isMaximized() || isMinimized())
        return;
    setMode(false);
    const auto factor = (96. / hostDpi) * percent / 100.;
    changing = true;
    auto target = hostBounds.size().toSize() * factor;
    if (target.width() < minimumSizeHint().width()) {
        target.setWidth(minimumSizeHint().width());
        target.setHeight(qRound(target.width() * hostBounds.height() / hostBounds.width()));
    }
    resize(target + QSize(0, bar->isHidden() ? 0 : bar->height()));
    changing = false;
    viewport.start();
}
void Viewer::setMode(bool host) {
    hostMode = host && connected && !readOnly && presentation["enabled"].toBool() &&
               presentation["can_resize"].toBool();
    mode->setCurrentIndex(hostMode ? 1 : 0);
    settle.stop();
    geometryPending = false;
    ignoreRemoteUntil = 0;
    if (!hostMode)
        fit();
    mode->setToolTip(text(hostMode ? "viewer.host_hint" : "viewer.scale_hint"));
}
void Viewer::updateWindow(QJsonObject entry, bool onlyRead, Viewer *parentViewer) {
    identity = QString::number(number(entry, "process_id")) + ":" +
               QString::number(number(entry, "thread_id"));
    auto previous = hostBounds;
    hostBounds = bounds(entry["bounds"].toObject());
    presentation = entry["presentation"].toObject();
    auto nextRole = presentation["role"].toString("independent");
    owner = number(presentation, "owner");
    bool first = previous.isEmpty();
    if (first || role != nextRole) {
        role = nextRole;
        Qt::WindowFlags flags = role == "independent" ? Qt::Window : Qt::Tool;
        if (role == "transient")
            flags |= Qt::FramelessWindowHint;
        setWindowFlags(flags);
        setNativeOwner(this, parentViewer, role == "independent");
    } else
        setNativeOwner(this, parentViewer, role == "independent");
    bar->setVisible(role != "transient");
    setWindowTitle(entry["title"].toString());
    readOnly = onlyRead;
    hostDpi = qBound(48., presentation["dpi"].toDouble(96), 768.);
    canvas->allowed = connected && !readOnly && presentation["enabled"].toBool() && !dismissed;
    if (!canvas->allowed)
        canvas->releaseInput();
    auto *items = qobject_cast<QStandardItemModel *>(mode->model());
    bool canHost = canvas->allowed && presentation["can_resize"].toBool();
    items->item(1)->setEnabled(canHost);
    if (hostMode && !canHost)
        setMode(false);
    QString why = readOnly                            ? text("viewer.host_read_only")
                  : !presentation["enabled"].toBool() ? text("viewer.host_blocked")
                                                      : text("viewer.host_fixed");
    mode->setToolTip(canHost ? text(hostMode ? "viewer.host_hint" : "viewer.scale_hint") : why);
    if (first) {
        changing = true;
        canvas->source = hostBounds.size().toSize();
        QSize view = hostBounds.size().toSize() * (96. / hostDpi);
        view = proportional(view.boundedTo(screen()->availableGeometry().size() - QSize(60, 100)),
                            canvas->source);
        resize(view + QSize(0, bar->isHidden() ? 0 : bar->sizeHint().height()));
        changing = false;
        if (parentViewer)
            move(parentViewer->pos() +
                 QPoint(qRound((hostBounds.x() - parentViewer->hostBounds.x()) * 96 / hostDpi),
                        qRound((hostBounds.y() - parentViewer->hostBounds.y()) * 96 / hostDpi)));
    } else if (hostMode && previous.size() != hostBounds.size() && !geometryPending &&
               QDateTime::currentMSecsSinceEpoch() > ignoreRemoteUntil && !isMaximized()) {
        changing = true;
        resize(QSize(qRound(canvas->width() * hostBounds.width() / previous.width()),
                     qRound(canvas->height() * hostBounds.height() / previous.height())) +
               QSize(0, bar->isHidden() ? 0 : bar->height()));
        changing = false;
    }
    if (!dismissed && !isVisible())
        show();
}
bool Viewer::frame(QJsonObject packet) {
    auto sequence = number(packet, "sequence");
    if (sequence <= receivedSequence)
        return true;
    receivedSequence = sequence;
    packet["_local_generation"] = qint64(localGeneration);
    return player->submit(packet);
}
void Viewer::feedback(QJsonObject p, double decode) {
    double now = double(std::chrono::duration_cast<std::chrono::milliseconds>(
                            std::chrono::steady_clock::now().time_since_epoch())
                            .count());
    QJsonObject ack{{"command", "video_feedback"},
                    {"stream_epoch", p["stream_epoch"]},
                    {"sequence", p["sequence"]},
                    {"decode_ms", decode},
                    {"queue_ms", qBound(0., now - p["_received"].toDouble(now) - decode, 60000.)},
                    {"total_ms", qBound(0., now - p["_received"].toDouble(now), 60000.)}};
    if (dismissed || isMinimized()) {
        if (held.size() < 2)
            held.append(ack);
    } else
        emit request(ack);
}
void Viewer::showLocal() {
    dismissed = false;
    canvas->allowed = connected && !readOnly && presentation["enabled"].toBool();
    showNormal();
    layout()->activate();
    if (!hostMode)
        fit();
    raise();
    activateWindow();
    canvas->setFocus();
    for (auto p : held)
        emit request(p);
    held.clear();
}
void Viewer::dismiss() {
    canvas->releaseInput();
    dismissed = true;
    canvas->allowed = false;
    hide();
}
void Viewer::offline() {
    canvas->releaseInput();
    connected = false;
    ++localGeneration;
    receivedSequence = 0;
    canvas->allowed = false;
    settle.stop();
    viewport.stop();
    held.clear();
    statistics->clear();
    statsAt = 0;
    presented = 0;
    canvas->image = {};
    canvas->update();
}
void Viewer::fit() {
    if (changing || dismissed || hostMode || canvas->source.isEmpty() || isMaximized() ||
        isMinimized())
        return;
    layout()->activate();
    changing = true;
    auto target = proportional(canvas->size(), canvas->source);
    resize(target + QSize(0, bar->isHidden() ? 0 : bar->height()));
    changing = false;
}
void Viewer::resizeEvent(QResizeEvent *e) {
    QWidget::resizeEvent(e);
    viewport.start();
    if (changing || isMaximized() || isMinimized())
        return;
    if (!hostMode) {
        if (canvas->source.isEmpty())
            return;
        const auto toolbar = bar->isHidden() ? 0 : bar->height();
        auto now = e->size() - QSize(0, toolbar), old = e->oldSize() - QSize(0, toolbar);
        const auto ratio = double(canvas->source.width()) / canvas->source.height();
        if (qAbs(double(now.width() - old.width())) / qMax(1, old.width()) >=
            qAbs(double(now.height() - old.height())) / qMax(1, old.height()))
            now.setHeight(qRound(now.width() / ratio));
        else
            now.setWidth(qRound(now.height() * ratio));
        QTimer::singleShot(0, this, [this, now, toolbar] {
            if (hostMode || changing || isMaximized())
                return;
            changing = true;
            resize(now + QSize(0, toolbar));
            changing = false;
        });
        return;
    }
    if (!geometryPending) {
        initialContent = e->oldSize() - QSize(0, bar->isHidden() ? 0 : bar->height());
        initialHost = hostBounds;
        geometryPending = true;
    }
    settle.start();
}
void Viewer::closeEvent(QCloseEvent *event) {
    event->ignore();
    dismiss();
    emit localClosed(id);
}
void Viewer::changeEvent(QEvent *e) {
    if (e->type() == QEvent::WindowStateChange && !changing) {
        settle.stop();
        geometryPending = false;
        changing = true;
        if (QGuiApplication::platformName().startsWith("wayland")) {
            // xdg-shell owns maximized geometry. Never submit a different
            // surface size while acknowledging that compositor state.
            QTimer::singleShot(0, this, [this] {
                changing = false;
                if (!isMinimized()) {
                    for (auto p : held)
                        emit request(p);
                    held.clear();
                }
            });
            QWidget::changeEvent(e);
            return;
        }
        bool maximize = isMaximized();
        if (maximize && !zoomed)
            beforeZoom = normalGeometry();
        QTimer::singleShot(0, this, [this, maximize] {
            if (maximize) {
                setWindowState(Qt::WindowNoState);
                if (zoomed) {
                    setGeometry(beforeZoom);
                    zoomed = false;
                } else {
                    auto area = screen()->availableGeometry();
                    auto chrome = frameGeometry().size() - size();
                    auto barSize = QSize(0, bar->isHidden() ? 0 : bar->height());
                    auto content = proportional(area.size() - chrome - barSize, canvas->source);
                    setGeometry(QRect(area.topLeft() + QPoint(chrome.width() / 2, chrome.height()),
                                      content + barSize));
                    zoomed = true;
                }
            }
            changing = false;
            if (!isMinimized()) {
                for (auto p : held)
                    emit request(p);
                held.clear();
                if (!hostMode)
                    fit();
            }
        });
    }
    QWidget::changeEvent(e);
}
bool Viewer::nativeEvent(const QByteArray &type, void *message, qintptr *result) {
    if (!hostMode &&
        constrainNativeResize(this, message, canvas->source, bar->isHidden() ? 0 : bar->height())) {
        *result = 1;
        return true;
    }
    return QWidget::nativeEvent(type, message, result);
}
void Viewer::resizeAck(QJsonObject response) {
    if (number(response, "revision") == revision)
        ignoreRemoteUntil = QDateTime::currentMSecsSinceEpoch() + 250;
}
} // namespace kokoda
