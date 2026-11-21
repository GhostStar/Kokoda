#pragma once
#include <QImage>
#include <QJsonObject>
#include <QObject>
#include <QThread>
#include <memory>

namespace kokoda {
class Decoder : public QObject {
    Q_OBJECT
    struct State;
    std::unique_ptr<State> state;

  public:
    Decoder();
    ~Decoder() override;
  public slots:
    void decode(QJsonObject packet);
  signals:
    void decoded(QJsonObject packet, QImage image, double milliseconds, QString error);
};
QImage checkedImage(const QByteArray &data);
class Player : public QObject {
    Q_OBJECT
    QThread thread;
    Decoder *decoder;
    int queued = 0;
    qint64 bytesQueued = 0;

  public:
    explicit Player(QObject *parent = nullptr);
    ~Player() override;
    bool submit(const QJsonObject &packet);
  signals:
    void enqueue(QJsonObject packet);
    void decoded(QJsonObject packet, QImage image, double milliseconds, QString error);
};
} // namespace kokoda
