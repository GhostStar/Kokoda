#pragma once
#include "core.h"
#include "decoder.h"
#include <QComboBox>
#include <QJsonArray>
#include <QLabel>
#include <QPointer>
#include <QSet>
#include <QTimer>
#include <QWidget>

namespace kokoda {
class Canvas : public QWidget {
    Q_OBJECT
    QPointF dragOrigin;
    QSize dragPixels, dragView;
    quint64 gesture = 0;
    QSet<int> pressed;
    QSet<int> buttons;
    QString preedit;
    QJsonObject point(QPointF location, const char *kind) const;
    QJsonObject delta(QPointF global, const char *kind) const;

  public:
    explicit Canvas(QWidget *parent = nullptr);
    QImage image;
    QSize source;
    QPointF caret = {0.05, 0.1};
    bool allowed = false, anchored = true, positionedWheel = false;
    QRectF imageRect() const;
    void releaseInput();
    QVariant inputMethodQuery(Qt::InputMethodQuery query) const override;
  signals:
    void input(QJsonObject event);
    void clipboard(QString action);
    void focused();

  protected:
    bool event(QEvent *) override;
    void paintEvent(QPaintEvent *) override;
    void inputMethodEvent(QInputMethodEvent *) override;
    void keyPressEvent(QKeyEvent *) override;
    void keyReleaseEvent(QKeyEvent *) override;
    void mousePressEvent(QMouseEvent *) override;
    void mouseDoubleClickEvent(QMouseEvent *e) override {
        mousePressEvent(e);
    }
    void mouseMoveEvent(QMouseEvent *) override;
    void mouseReleaseEvent(QMouseEvent *) override;
    void wheelEvent(QWheelEvent *) override;
    void focusInEvent(QFocusEvent *) override;
    void focusOutEvent(QFocusEvent *) override;
};
class Viewer : public QWidget {
    Q_OBJECT
    QTimer settle, viewport;
    QLabel *statistics;
    qint64 statsAt = 0;
    int presented = 0;
    bool adaptive = true;
    bool changing = false, geometryPending = false, zoomed = false;
    QRect beforeZoom;
    quint64 localGeneration = 0;
    qint64 ignoreRemoteUntil = 0;
    QSize initialContent;
    QRectF initialHost;
    quint64 revision = 0, receivedSequence = 0;
    QVector<QJsonObject> held;
    void fit();
    void feedback(QJsonObject packet, double decode);

  public:
    quint64 sid, id, token;
    QString role = "independent";
    QString identity;
    quint64 owner = 0;
    QRectF hostBounds;
    double hostDpi = 96;
    QJsonObject presentation;
    bool dismissed = false, connected = true, readOnly = false, hostMode = false;
    Canvas *canvas;
    QComboBox *mode;
    QWidget *bar;
    Player *player;
    explicit Viewer(quint64 session, quint64 window, quint64 generation);
    ~Viewer() override;
    void updateWindow(QJsonObject entry, bool readOnly, Viewer *owner);
    bool frame(QJsonObject packet);
    void showLocal();
    void dismiss();
    void offline();
    void setMode(bool host);
    void setDisplayScale(int percent);
    void setAdaptiveResolution(bool enabled);
    void resizeAck(QJsonObject response);
  signals:
    void request(QJsonObject packet);
    void localClosed(quint64 id);
    void clipboard(QString action);
    void focused();
    void failed(QString reason);

  protected:
    void resizeEvent(QResizeEvent *) override;
    void closeEvent(QCloseEvent *) override;
    void changeEvent(QEvent *) override;
    bool nativeEvent(const QByteArray &, void *, qintptr *) override;
};
} // namespace kokoda
