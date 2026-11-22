#pragma once
#include "connection.h"
#include "core.h"
#include "viewer.h"
#include <QMap>
#include <QPointer>
#include <QQueue>
#include <QTimer>
#include <memory>

namespace kokoda {
struct AppSession {
    quint64 sid = 0, root = 0;
    QString ticket;
    bool selected = false, application = true, qualityPending = true;
    QMap<quint64, Viewer *> viewers;
    ~AppSession() {
        qDeleteAll(viewers);
    }
};
class Session : public QObject {
    Q_OBJECT
    Core core;
    Connection *connection = nullptr;
    QQueue<QJsonObject> queued, pending;
    QTimer maintenance, retry;
    QString address;
    QByteArray password;
    quint64 generation = 0, nextSession = 0, nextClipboard = 0;
    int retryCount = 0, clipboardVersion = 0;
    bool wanted = false, streamStarted = false;
    qint64 lastHeartbeat = 0, lastGroup = 0, lastClipboard = 0;
    QPointer<Viewer> active;
    bool clipboardBaseline = true;
    struct ClipboardRequest {
        QPointer<Viewer> viewer;
        int version;
    };
    QMap<quint64, ClipboardRequest> copies;
    void begin();
    void lost(QString category, bool retryable);
    void pump();
    void tick();
    void group(AppSession &state, QJsonObject message);
    void remove(quint64 sid);

  public:
    QMap<quint64, std::shared_ptr<AppSession>> apps;
    QJsonArray catalog;
    int preset = 2;
    bool positionedWheel = false;
    bool authenticated = false, readOnly = true, syncClipboard = true, includeApplication = true;
    bool adaptiveResolution = true, autoReconnect = true;
    explicit Session(QObject *parent = nullptr);
    ~Session() override;
    bool valid() const {
        return core.valid();
    }
    void connectHost(QString host, QByteArray secret);
    void disconnectHost();
    void refresh();
    void reconnectHost();
    void setAdaptiveResolution(bool enabled);
    void open(quint64 id);
    void end(quint64 sid);
    void reopen();
    void setDisplayScale(int percent);
    void setQuality(int value);
    void send(QJsonObject message);
    void receive(QJsonObject message);
    void clipboard(Viewer *viewer, QString action);
  signals:
    void changed();
    void status(QString message);
    // Test observations exclude passwords and clipboard content.
    void outgoing(QJsonObject message);
};
} // namespace kokoda
