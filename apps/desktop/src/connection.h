#pragma once
#include <QElapsedTimer>
#include <QJsonObject>
#include <QObject>
#include <QProcess>
#include <QThread>
#include <QTimer>

namespace kokoda {
// QProcess and JSON parsing stay off the GUI thread. Rust owns TLS/OPAQUE and FIFO correlation.
class Wire : public QObject {
    Q_OBJECT
    QProcess *process = nullptr;
    QByteArray buffer;
    QTimer *watchdog = nullptr;
    QElapsedTimer received;
    bool stopped = false;
    qint64 deliveryBytes = 0;
    int deliveries = 0;
    void fail(const QString &category, bool retryable);
  public slots:
    void start(QString address, QByteArray password);
    void send(QJsonObject packet);
    void stop();
    void delivered(qint64 bytes);
  signals:
    void packet(QJsonObject packet);
    void lost(QString category, bool retryable);
};
class Connection : public QObject {
    Q_OBJECT
    QThread thread;
    Wire *wire;

  public:
    explicit Connection(QObject *parent = nullptr);
    ~Connection() override;
    void close();
  signals:
    void start(QString address, QByteArray password);
    void send(QJsonObject packet);
    void packet(QJsonObject packet);
    void delivered(qint64 bytes);
    void lost(QString category, bool retryable);
};
} // namespace kokoda
