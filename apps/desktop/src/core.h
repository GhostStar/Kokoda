#pragma once
#include <QJsonObject>
#include <QLibrary>
#include <QRectF>
#include <QString>

namespace kokoda {
QString text(const char *key);
void setLanguage(const QString &language);
class Core {
    QLibrary library;
    void *handle = nullptr;
    void (*release)(void *) = nullptr;
    char *(*dispatch)(void *, const char *) = nullptr;
    void (*releaseString)(char *) = nullptr;

  public:
    Core();
    ~Core();
    bool valid() const {
        return handle != nullptr;
    }
    QJsonObject action(QJsonObject event);
    QJsonObject profile(int preset);
};
quint64 number(const QJsonObject &object, const char *key);
QRectF bounds(const QJsonObject &object);
QSize proportional(QSize available, QSize source);
} // namespace kokoda
