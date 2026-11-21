#include "core.h"
#include <QCoreApplication>
#include <QFile>
#include <QJsonDocument>
#include <QLocale>
#include <algorithm>

static void resources() {
    Q_INIT_RESOURCE(resources);
}

namespace kokoda {
static QJsonObject strings;
void setLanguage(const QString &language) {
    resources();
    QFile file(language.startsWith("zh") ? ":/locales/zh-CN.json" : ":/locales/en.json");
    if (file.open(QIODevice::ReadOnly))
        strings = QJsonDocument::fromJson(file.readAll()).object();
}
QString text(const char *key) {
    if (strings.isEmpty())
        setLanguage(QLocale::system().name());
    return strings.value(QLatin1String(key)).toString(QLatin1String(key));
}
Core::Core() : library(QCoreApplication::applicationDirPath() + "/kokoda_client") {
    // Rust's runtime can register thread-local cleanup callbacks. Keep the
    // module mapped until process exit, even when the last controller is freed.
    library.setLoadHints(QLibrary::PreventUnloadHint);
    if (!library.load())
        return;
    auto create = reinterpret_cast<void *(*)()>(library.resolve("kokoda_client_new"));
    release = reinterpret_cast<void (*)(void *)>(library.resolve("kokoda_client_free"));
    dispatch = reinterpret_cast<char *(*)(void *, const char *)>(
        library.resolve("kokoda_client_dispatch"));
    releaseString =
        reinterpret_cast<void (*)(char *)>(library.resolve("kokoda_client_string_free"));
    if (create && release && dispatch && releaseString)
        handle = create();
}
Core::~Core() {
    if (handle)
        release(handle);
}
QJsonObject Core::action(QJsonObject event) {
    if (!handle)
        return {};
    auto input = QJsonDocument(event).toJson(QJsonDocument::Compact);
    char *result = dispatch(handle, input.constData());
    auto object = QJsonDocument::fromJson(result ? QByteArray(result) : QByteArray()).object();
    if (result)
        releaseString(result);
    return object;
}
QJsonObject Core::profile(int preset) {
    return action({{"type", "profile"}, {"preset", preset}});
}
quint64 number(const QJsonObject &o, const char *k) {
    return o.value(QLatin1String(k)).toVariant().toULongLong();
}
QRectF bounds(const QJsonObject &o) {
    return {o["left"].toDouble(), o["top"].toDouble(), o["right"].toDouble() - o["left"].toDouble(),
            o["bottom"].toDouble() - o["top"].toDouble()};
}
QSize proportional(QSize available, QSize source) {
    if (source.isEmpty())
        return available;
    return source.scaled(available, Qt::KeepAspectRatio);
}
} // namespace kokoda
