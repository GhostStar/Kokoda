#include "core.h"
#include "mainwindow.h"
#include "session.h"
#include "viewer.h"
#include <QBuffer>
#include <QClipboard>
#include <QFile>
#include <QInputMethodEvent>
#include <QJsonDocument>
#include <QSettings>
#include <QTemporaryDir>
#include <QUuid>
#include <QtTest>
using namespace kokoda;
class ClientTests : public QObject {
    Q_OBJECT
    QJsonObject entry(quint64 id, QString role = "independent", quint64 owner = 0) {
        return {
            {"id", qint64(id)},
            {"token", qint64(id)},
            {"title", "Kokoda fixture"},
            {"bounds", QJsonObject{{"left", 100}, {"top", 100}, {"right", 700}, {"bottom", 500}}},
            {"presentation", QJsonObject{{"role", role},
                                         {"owner", qint64(owner)},
                                         {"enabled", true},
                                         {"can_resize", true},
                                         {"dpi", 144}}}};
    }
  private slots:
    void sharedCore() {
        Core core;
        QVERIFY(core.valid());
        QCOMPARE(core.profile(2)["bitrate"].toInt(), 6000000);
        auto a = core.action({{"type", "track"}, {"session", 1}, {"id", 10}, {"generation", 1}});
        QCOMPARE(a["actions"].toArray().size(), 1);
    }
    void languages() {
        setLanguage("zh-CN");
        QCOMPARE(text("viewer.scale_local"), QString::fromUtf8("缩放画面"));
        setLanguage("en");
        QCOMPARE(text("viewer.scale_local"), QString("Scale view"));
    }
    void mainInterface() {
        for (auto language : {"en", "zh-CN"}) {
            setLanguage(language);
            MainWindow window;
            window.show();
            QVERIFY(QTest::qWaitForWindowExposed(&window));
            const auto directory = qEnvironmentVariable("KOKODA_TEST_SCREENSHOTS");
            if (!directory.isEmpty())
                QVERIFY(window.grab().save(directory + "/desktop-" + language + ".png"));
            QVERIFY(window.session.valid());
        }
        setLanguage("en");
    }
    void composedInput() {
        Canvas canvas;
        canvas.allowed = true;
        canvas.resize(600, 400);
        QSignalSpy input(&canvas, &Canvas::input);
        QInputMethodEvent composing("zhong", {});
        QApplication::sendEvent(&canvas, &composing);
        QCOMPARE(input.count(), 0);
        QInputMethodEvent commit;
        commit.setCommitString(QString::fromUtf8("中文🙂"));
        QApplication::sendEvent(&canvas, &commit);
        QCOMPARE(input.count(), 1);
        QCOMPARE(input[0][0].toJsonObject()["text"].toString(), QString::fromUtf8("中文🙂"));
        QVERIFY(canvas.inputMethodQuery(Qt::ImCursorRectangle).toRectF().isValid());
    }
    void localControls() {
        Viewer viewer(1, 10, 10);
        viewer.updateWindow(entry(10), false, nullptr);
        QVERIFY(QTest::qWaitForWindowExposed(&viewer));
        QSignalSpy requests(&viewer, &Viewer::request);
        viewer.resize(720, 500);
        QTest::qWait(300);
        QVERIFY(qAbs(double(viewer.canvas->width()) / viewer.canvas->height() - 1.5) < 0.02);
        for (auto row : requests)
            QVERIFY(row[0].toJsonObject()["command"] != "window_action");
        viewer.setDisplayScale(150);
        QTest::qWait(100);
        QVERIFY(qAbs(viewer.canvas->width() - 600) < 3);
        QVERIFY(qAbs(double(viewer.canvas->width()) / viewer.canvas->height() - 1.5) < 0.02);
        viewer.close();
        QVERIFY(viewer.dismissed);
        QVERIFY(!viewer.isVisible());
        viewer.updateWindow(entry(10), false, nullptr);
        QVERIFY(!viewer.isVisible());
        viewer.showLocal();
        QVERIFY(viewer.isVisible());
    }
    void hostResize() {
        Viewer viewer(1, 10, 10);
        viewer.updateWindow(entry(10), false, nullptr);
        viewer.show();
        QTest::qWait(50);
        viewer.setMode(true);
        QSignalSpy requests(&viewer, &Viewer::request);
        viewer.resize(800, 500);
        QTest::qWait(350);
        bool found = false;
        for (auto row : requests) {
            auto p = row[0].toJsonObject();
            if (p["command"] == "window_action") {
                found = true;
                QCOMPARE(p["action"].toString(), QString("resize"));
            }
        }
        QVERIFY(found);
        viewer.setMode(false);
        QTest::qWait(50);
        QVERIFY(qAbs(double(viewer.canvas->width()) / viewer.canvas->height() - 1.5) < 0.02);
    }
    void readOnly() {
        Viewer viewer(1, 10, 10);
        viewer.updateWindow(entry(10), true, nullptr);
        viewer.setMode(true);
        QVERIFY(!viewer.hostMode);
        QVERIFY(!viewer.canvas->allowed);
        viewer.close();
        QVERIFY(viewer.dismissed);
    }
    void nativeZoomStaysLocal() {
        Viewer viewer(1, 10, 10);
        viewer.updateWindow(entry(10), false, nullptr);
        QTest::qWait(50);
        viewer.setMode(true);
        QSignalSpy requests(&viewer, &Viewer::request);
        viewer.showMaximized();
        QTest::qWait(400);
        for (auto row : requests)
            QVERIFY(row[0].toJsonObject()["command"] != "window_action");
        auto picture = viewer.canvas->imageRect();
        if (!QGuiApplication::platformName().startsWith("wayland")) {
            QVERIFY(qAbs(picture.width() - viewer.canvas->width()) < 2);
            QVERIFY(qAbs(picture.height() - viewer.canvas->height()) < 2);
        }
        QVERIFY(qAbs(picture.width() / picture.height() - 1.5) < 0.01);
        requests.clear();
        viewer.showMaximized();
        QTest::qWait(400);
        for (auto row : requests)
            QVERIFY(row[0].toJsonObject()["command"] != "window_action");
    }
    void focusLossReleasesButtons() {
        Canvas canvas;
        canvas.allowed = true;
        canvas.source = {600, 400};
        canvas.resize(600, 400);
        canvas.show();
        QSignalSpy events(&canvas, &Canvas::input);
        QTest::mousePress(&canvas, Qt::RightButton, Qt::NoModifier, QPoint(40, 40));
        canvas.releaseInput();
        QCOMPARE(events.last()[0].toJsonObject()["kind"].toString(), QString("release"));
    }
    void reconnectPreferences() {
        Session s;
        s.authenticated = true;
        auto app = std::make_shared<AppSession>();
        app->sid = 1;
        app->root = 10;
        app->qualityPending = false;
        s.apps[1] = app;
        auto original = entry(10);
        s.receive({{"type", "window_group"},
                   {"session_id", 1},
                   {"root_id", 10},
                   {"windows", QJsonArray{original}}});
        auto *viewer = app->viewers[10];
        viewer->close();
        viewer->offline();
        auto geometry = viewer->geometry();
        original["token"] = 22;
        s.receive({{"type", "window_group"},
                   {"session_id", 1},
                   {"root_id", 10},
                   {"windows", QJsonArray{original}}});
        QVERIFY(app->viewers[10]->dismissed);
        QCOMPARE(app->viewers[10]->geometry(), geometry);
        original["token"] = 23;
        s.receive({{"type", "window_group"},
                   {"session_id", 1},
                   {"root_id", 10},
                   {"windows", QJsonArray{original}}});
        QVERIFY(!app->viewers[10]->dismissed);
    }
    void windows() {
        Session s;
        s.authenticated = true;
        auto app = std::make_shared<AppSession>();
        app->sid = 1;
        app->root = 10;
        app->qualityPending = false;
        s.apps[1] = app;
        s.receive({{"type", "window_group"},
                   {"session_id", 1},
                   {"root_id", 10},
                   {"windows", QJsonArray{entry(10), entry(11, "floating", 10), entry(12)}}});
        QCOMPARE(app->viewers.size(), 3);
        app->viewers[10]->close();
        QVERIFY(app->viewers[11]->dismissed);
        QVERIFY(!app->viewers[12]->dismissed);
        s.reopen();
        QVERIFY(!app->viewers[11]->dismissed);
    }
    void decodeImage() {
        QImage image(120, 80, QImage::Format_RGB32);
        image.fill(Qt::red);
        QByteArray bytes;
        QBuffer buffer(&bytes);
        buffer.open(QIODevice::WriteOnly);
        QVERIFY(image.save(&buffer, "PNG"));
        QCOMPARE(checkedImage(bytes).size(), image.size());
        QVERIFY(checkedImage("bad").isNull());
        Player player;
        QSignalSpy decoded(&player, &Player::decoded);
        QVERIFY(
            player.submit({{"format", "png"}, {"image", QString::fromLatin1(bytes.toBase64())}}));
        QTRY_COMPARE(decoded.count(), 1);
        QCOMPARE(qvariant_cast<QImage>(decoded[0][1]).pixelColor(0, 0), QColor(Qt::red));
    }
    void decodePaddedVideo() {
        QFile file(QStringLiteral(KOKODA_FIXTURE_DIR) + "/desktop-color.h264");
        QVERIFY(file.open(QIODevice::ReadOnly));
        const auto encoded = QString::fromLatin1(file.readAll().toBase64());
        Player player;
        QSignalSpy decoded(&player, &Player::decoded);
        for (int i = 0; i < 24; ++i) {
            QVERIFY(player.submit({{"format", "h264"},
                                   {"image", encoded},
                                   {"stream_epoch", i / 8},
                                   {"sequence", i + 1}}));
            QTRY_COMPARE_WITH_TIMEOUT(decoded.count(), i + 1, 5000);
            QVERIFY2(decoded[i][3].toString().isEmpty(), qPrintable(decoded[i][3].toString()));
            auto image = qvariant_cast<QImage>(decoded[i][1]);
            QCOMPARE(image.size(), QSize(98, 66));
            auto color = image.pixelColor(97, 65);
            QVERIFY(color.red() < 5 && qAbs(color.green() - 128) < 5 &&
                    qAbs(color.blue() - 128) < 5);
        }
    }
    void live() {
        const auto address = qEnvironmentVariable("KOKODA_TEST_HOST");
        if (address.isEmpty())
            QSKIP("Set KOKODA_TEST_HOST and KOKODA_TEST_SECRET for an isolated live fixture");
        QFile secret(qEnvironmentVariable("KOKODA_TEST_SECRET"));
        QVERIFY(secret.open(QIODevice::ReadOnly));
        auto password =
            QJsonDocument::fromJson(secret.readAll()).object()["password"].toString().toUtf8();
        secret.close();
        qInfo("Live: constructing session");
        Session s;
        qInfo("Live: connecting");
        QSignalSpy status(&s, &Session::status);
        s.connectHost(address, password);
        password.fill('\0');
        QTRY_VERIFY_WITH_TIMEOUT(s.authenticated, 20000);
        qInfo("Live: authenticated");
        quint64 root = 0;
        QTRY_VERIFY_WITH_TIMEOUT(([&] {
                                     for (auto v : s.catalog)
                                         if (v.toObject()["title"] == "Kokoda Roles Root")
                                             root = number(v.toObject(), "id");
                                     return root != 0;
                                 })(),
                                 20000);
        QTest::qWait(300);
        s.open(root);
        qInfo("Live: opening fixture");
        QTRY_VERIFY_WITH_TIMEOUT(s.apps.size() == 1 && s.apps.first()->viewers.size() == 5, 20000);
        auto app = s.apps.first();
        qInfo("Live: viewers created");
        QTRY_VERIFY_WITH_TIMEOUT(([&] {
                                     for (auto *v : app->viewers)
                                         if (v->canvas->image.isNull())
                                             return false;
                                     return true;
                                 })(),
                                 20000);
        Viewer *tool = nullptr;
        for (auto *v : app->viewers)
            if (v->windowTitle() == "Kokoda Roles Tool")
                tool = v;
        QVERIFY(tool);
        auto original = tool->hostBounds;
        tool->resize(tool->size() + QSize(80, 0));
        QTest::qWait(600);
        QCOMPARE(tool->hostBounds, original);
        auto *rootViewer = app->viewers[root];
        const auto fixture = qEnvironmentVariable("KOKODA_TEST_FIXTURE_DIR");
        if (!fixture.isEmpty()) {
            int commandSerial = 0;
            QByteArray sent;
            const auto runId = QUuid::createUuid().toByteArray();
            auto command = [&](const char *value) {
                QFile file(fixture + "/roles-command.txt");
                if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate))
                    return false;
                sent = runId + QByteArray::number(++commandSerial) + ":" + value;
                file.write(sent);
                return true;
            };
            auto acknowledged = [&] {
                QFile file(fixture + "/roles-state.json");
                if (!file.open(QIODevice::ReadOnly)) return false;
                auto state = QJsonDocument::fromJson(file.readAll()).object();
                return QByteArray::fromBase64(state["command_base64"].toString().toLatin1()) == sent;
            };
            auto editor = [&] {
                QFile file(fixture + "/roles-state.json");
                if (!file.open(QIODevice::ReadOnly))
                    return QString();
                auto state = QJsonDocument::fromJson(file.readAll()).object();
                return QString::fromUtf8(
                    QByteArray::fromBase64(state["editor_base64"].toString().toLatin1()));
            };
            QVERIFY(command("clear-editor"));
            QTRY_VERIFY_WITH_TIMEOUT(acknowledged(), 10000);
            QTRY_COMPARE_WITH_TIMEOUT(editor(), QString(), 10000);
            QInputMethodEvent commit;
            auto value = QString::fromUtf8("Kokoda 中文🙂");
            commit.setCommitString(value);
            QApplication::sendEvent(rootViewer->canvas, &commit);
            QTRY_COMPARE_WITH_TIMEOUT(editor(), value, 10000);
            auto paste = QString::fromUtf8(" / paste 粘贴");
            QApplication::clipboard()->setText(paste);
            s.clipboard(rootViewer, "paste");
            QTRY_COMPARE_WITH_TIMEOUT(editor(), value + paste, 10000);
#ifndef Q_OS_WIN
            rootViewer->showLocal();
            QTRY_VERIFY(rootViewer->isActiveWindow());
            QTest::qWait(900); // establish the contextual clipboard baseline
            QApplication::clipboard()->setText("local baseline");
            QVERIFY(command("copy-text"));
            QTRY_VERIFY_WITH_TIMEOUT(acknowledged(), 10000);
            QTRY_COMPARE_WITH_TIMEOUT(QApplication::clipboard()->text(),
                                      QString::fromUtf8("Kokoda clipboard 中文"), 10000);
            QVERIFY(command("copy-image"));
            QTRY_VERIFY_WITH_TIMEOUT(acknowledged(), 10000);
            QTRY_COMPARE_WITH_TIMEOUT(QApplication::clipboard()->image().size(), QSize(32, 24),
                                      10000);
            QCOMPARE(QApplication::clipboard()->image().pixelColor(0, 0), QColor(33, 177, 199));
#endif
        }
        QSignalSpy decoded(rootViewer->player, &Player::decoded);
        s.setQuality(0);
        QTRY_VERIFY_WITH_TIMEOUT(decoded.count() > 0, 15000);
        decoded.clear();
        s.setQuality(2);
        QTRY_VERIFY_WITH_TIMEOUT(decoded.count() > 0, 15000);
        QVERIFY(s.authenticated);
        QCOMPARE(app->viewers[root], rootViewer);
        tool->setMode(true);
        tool->resize(tool->size() + QSize(80, 40));
        QTRY_VERIFY_WITH_TIMEOUT(tool->hostBounds.size() != original.size(), 10000);
        tool->close();
        QVERIFY(tool->dismissed);
        QTest::qWait(700);
        QVERIFY(!tool->isVisible());
        s.reopen();
        QVERIFY(tool->isVisible());
        auto savedGeometry = rootViewer->geometry();
        s.reconnectHost();
        QTRY_VERIFY_WITH_TIMEOUT(
            s.apps.isEmpty() ||
                (s.authenticated && app->viewers.contains(root) && app->viewers[root]->connected),
            20000);
        QVERIFY(!s.apps.isEmpty());
        QTRY_VERIFY_WITH_TIMEOUT(!app->viewers[root]->canvas->image.isNull(), 20000);
        QCOMPARE(app->viewers[root]->geometry(), savedGeometry);
        s.disconnectHost();
        QVERIFY(s.apps.isEmpty());
    }
};
int main(int argc, char **argv) {
    QApplication app(argc, argv);
    QTemporaryDir settings;
    if (!settings.isValid())
        return 2;
    QCoreApplication::setOrganizationName("KokodaTests");
    QCoreApplication::setApplicationName("KokodaDesktopTests");
    QSettings::setDefaultFormat(QSettings::IniFormat);
    QSettings::setPath(QSettings::IniFormat, QSettings::UserScope, settings.path());
    ClientTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "client_tests.moc"
