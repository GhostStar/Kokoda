#include "connection.h"
#include <QCoreApplication>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonParseError>
#include <chrono>

namespace kokoda {
void Wire::start(QString address, QByteArray password) {
    process = new QProcess(this);
    auto executable = QCoreApplication::applicationDirPath() + "/kokoda-bridge";
#ifdef Q_OS_WIN
    executable += ".exe";
    process->setCreateProcessArgumentsModifier(
        [](QProcess::CreateProcessArguments *a) { a->flags |= 0x08000000; });
#endif
    connect(process, &QProcess::readyReadStandardError, this,
            [this] { process->readAllStandardError(); });
    connect(process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError e) {
        fail(e == QProcess::FailedToStart ? "bridge_missing" : "network",
             e != QProcess::FailedToStart);
    });
    connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
            [this] { fail("network", true); });
    connect(process, &QProcess::readyReadStandardOutput, this, [this] {
        buffer += process->readAllStandardOutput();
        if (buffer.size() > 64 * 1024 * 1024) {
            fail("protocol", false);
            return;
        }
        while (!stopped) {
            auto end = buffer.indexOf('\n');
            if (end < 0)
                break;
            QJsonParseError error;
            auto doc = QJsonDocument::fromJson(buffer.left(end), &error);
            buffer.remove(0, end + 1);
            if (error.error != QJsonParseError::NoError || !doc.isObject()) {
                fail("protocol", false);
                return;
            }
            received.restart();
            auto value = doc.object();
            if (value["type"] == "bridge_error") {
                fail(value["category"].toString(), value["retryable"].toBool());
                return;
            }
            value["_received"] = double(std::chrono::duration_cast<std::chrono::milliseconds>(
                                            std::chrono::steady_clock::now().time_since_epoch())
                                            .count());
            deliveryBytes += 2 * end;
            if (++deliveries > 256 || deliveryBytes > 96 * 1024 * 1024) {
                fail("backlog", false);
                return;
            }
            value["_wire_bytes"] = 2 * end;
            emit packet(value);
        }
    });
    process->start(executable, {address, "--password-stdin", "--stream-bridge"});
    // QProcess buffers writes until its child is ready; secrets never become argv or files.
    auto secret = QJsonDocument(QJsonObject{{"password", QString::fromUtf8(password)}})
                      .toJson(QJsonDocument::Compact);
    process->write(secret + '\n');
    secret.fill('\0');
    password.fill('\0');
    received.start();
    watchdog = new QTimer(this);
    watchdog->setInterval(1000);
    connect(watchdog, &QTimer::timeout, this, [this] {
        if (received.elapsed() > 35000)
            fail("network", true);
    });
    watchdog->start();
    send({{"command", "list_windows"}});
}
void Wire::delivered(qint64 bytes) {
    deliveryBytes -= bytes;
    --deliveries;
}
void Wire::send(QJsonObject packet) {
    if (stopped || !process)
        return;
    if (process->bytesToWrite() > 16 * 1024 * 1024) {
        fail("backlog", false);
        return;
    }
    process->write(QJsonDocument(packet).toJson(QJsonDocument::Compact) + '\n');
}
void Wire::fail(const QString &category, bool retryable) {
    if (stopped)
        return;
    stop();
    emit lost(category, retryable);
}
void Wire::stop() {
    stopped = true;
    if (watchdog)
        watchdog->stop();
    if (process) {
        process->closeWriteChannel();
        if (!process->waitForFinished(100)) {
            process->kill();
            process->waitForFinished(1000);
        }
    }
    buffer.clear();
}
Connection::Connection(QObject *parent) : QObject(parent), wire(new Wire) {
    wire->moveToThread(&thread);
    connect(&thread, &QThread::finished, wire, &QObject::deleteLater);
    connect(this, &Connection::start, wire, &Wire::start);
    connect(this, &Connection::send, wire, &Wire::send);
    connect(wire, &Wire::packet, this, [this](QJsonObject p) {
        emit delivered(p.take("_wire_bytes").toInteger());
        emit packet(p);
    });
    connect(this, &Connection::delivered, wire, &Wire::delivered);
    connect(wire, &Wire::lost, this, &Connection::lost);
    thread.start();
}
void Connection::close() {
    QMetaObject::invokeMethod(wire, &Wire::stop, Qt::BlockingQueuedConnection);
}
Connection::~Connection() {
    close();
    thread.quit();
    thread.wait();
}
} // namespace kokoda
