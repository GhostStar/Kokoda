#include "mainwindow.h"
#include <QApplication>
#include <QCloseEvent>
#include <QCommandLineParser>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QIcon>
#include <QJsonDocument>
#include <QLocale>
#include <QMenuBar>
#include <QMessageBox>
#include <QScrollBar>
#include <QSettings>
#include <QVBoxLayout>
#include <algorithm>
namespace kokoda {
MainWindow::MainWindow() {
    setWindowTitle("Kokoda");
    setWindowIcon(QIcon(":/kokoda.png"));
    resize(680, 680);
    auto *panel = new QWidget(this);
    setCentralWidget(panel);
    auto *layout = new QVBoxLayout(panel);
    layout->setContentsMargins(20, 20, 20, 20);
    layout->setSpacing(10);
    auto *form = new QFormLayout;
    host = new QLineEdit(QSettings().value("hostAddress").toString());
    host->setPlaceholderText("192.168.1.10:47832");
    password = new QLineEdit;
    password->setEchoMode(QLineEdit::Password);
    password->setMaxLength(256);
    form->addRow(text("connection.address"), host);
    form->addRow(text("connection.password"), password);
    layout->addLayout(form);
    auto *connectionRow = new QHBoxLayout;
    connectButton = new QPushButton(text("connection.connect"));
    auto *disconnectButton = new QPushButton(text("connection.disconnect"));
    connectionRow->addWidget(connectButton);
    connectionRow->addWidget(disconnectButton);
    connectionRow->addStretch();
    layout->addLayout(connectionRow);
    auto *quality = new QComboBox;
    for (auto key : {"quality.original", "quality.smooth", "quality.clear", "quality.fast"})
        quality->addItem(text(key));
    quality->setCurrentIndex(session.preset);
    quality->setToolTip(text("quality.tip"));
    quality->setAccessibleName(text("menu.quality"));
    layout->addWidget(quality);
    auto *adaptive = new QCheckBox(text("display.adaptive"));
    adaptive->setChecked(true);
    adaptive->setToolTip(text("display.adaptive_tip"));
    layout->addWidget(adaptive);
    connect(adaptive, &QCheckBox::toggled, &session, &Session::setAdaptiveResolution);
    auto *application = new QCheckBox(text("windows.include_app"));
    application->setChecked(true);
    layout->addWidget(application);
    auto *clipboard = new QCheckBox(text("clipboard.sync"));
    clipboard->setChecked(true);
    clipboard->setToolTip(text("clipboard.sync_tip"));
    layout->addWidget(clipboard);
    layout->addWidget(new QLabel(text("desktop.clipboard_shortcuts")));
    search = new QLineEdit;
    search->setPlaceholderText(text("desktop.search"));
    search->setAccessibleName(text("desktop.search"));
    layout->addWidget(search);
    windows = new QListWidget;
    windows->setAccessibleName(text("desktop.available_windows"));
    layout->addWidget(windows, 2);
    auto *actions = new QHBoxLayout;
    auto *refresh = new QPushButton(text("windows.refresh"));
    openButton = new QPushButton(text("windows.open"));
    openButton->setEnabled(false);
    actions->addWidget(refresh);
    actions->addWidget(openButton);
    layout->addLayout(actions);
    layout->addWidget(new QLabel(text("desktop.open_apps")));
    opened = new QListWidget;
    opened->setMaximumHeight(110);
    layout->addWidget(opened, 1);
    auto *end = new QPushButton(text("menu.end_viewing"));
    layout->addWidget(end);
    message = new QLabel(text("status.not_connected"));
    message->setWordWrap(true);
    message->setTextFormat(Qt::PlainText);
    message->setTextInteractionFlags(Qt::TextSelectableByMouse);
    layout->addWidget(message);
    auto connectHost = [this] {
        if (host->text().trimmed().isEmpty() || password->text().isEmpty())
            return;
        auto scalars = password->text().toUcs4();
        if (scalars.size() < 8 || scalars.size() > 128 ||
            std::any_of(scalars.begin(), scalars.end(),
                        [](char32_t c) { return QChar::category(c) == QChar::Other_Control; })) {
            message->setText(text("error.password_required"));
            return;
        }
        QSettings().setValue("hostAddress", host->text().trimmed());
        session.connectHost(host->text(), password->text().toUtf8());
        password->clear();
    };
    connect(connectButton, &QPushButton::clicked, this, connectHost);
    connect(password, &QLineEdit::returnPressed, this, connectHost);
    connect(disconnectButton, &QPushButton::clicked, &session, &Session::disconnectHost);
    connect(quality, qOverload<int>(&QComboBox::activated), &session, &Session::setQuality);
    connect(application, &QCheckBox::toggled, this,
            [this](bool on) { session.includeApplication = on; });
    connect(clipboard, &QCheckBox::toggled, this, [this](bool on) { session.syncClipboard = on; });
    connect(refresh, &QPushButton::clicked, &session, &Session::refresh);
    auto open = [this] {
        if (auto *item = windows->currentItem())
            session.open(item->data(Qt::UserRole).toULongLong());
    };
    connect(openButton, &QPushButton::clicked, this, open);
    connect(windows, &QListWidget::itemDoubleClicked, this, [open] { open(); });
    connect(end, &QPushButton::clicked, this, [this] {
        if (auto *item = opened->currentItem())
            session.end(item->data(Qt::UserRole).toULongLong());
    });
    connect(opened, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem *item) {
        auto state = session.apps.value(item->data(Qt::UserRole).toULongLong());
        if (state)
            session.open(state->root);
    });
    connect(search, &QLineEdit::textChanged, this, &MainWindow::updateCatalog);
    connect(&session, &Session::changed, this, &MainWindow::updateCatalog);
    connect(&session, &Session::status, message, &QLabel::setText);
    auto *connectionMenu = menuBar()->addMenu(text("menu.connection"));
    connectionMenu->addAction(text("menu.reconnect"), &session, &Session::reconnectHost);
    auto *automatic = connectionMenu->addAction(text("menu.auto_reconnect"));
    automatic->setCheckable(true);
    automatic->setChecked(true);
    connect(automatic, &QAction::toggled, this, [this](bool on) { session.autoReconnect = on; });
    connectionMenu->addAction(text("menu.disconnect"), &session, &Session::disconnectHost);
    connectionMenu->addAction(text("menu.refresh"), &session, &Session::refresh);
    connectionMenu->addAction(text("menu.quit"), QKeySequence::Quit, qApp, &QApplication::quit);
    auto *windowMenu = menuBar()->addMenu(text("menu.window"));
    auto *zoom = windowMenu->addMenu(text("menu.viewer_zoom"));
    for (int percent : {50, 75, 100, 125, 150, 200})
        zoom->addAction(QString::number(percent) + "%", &session,
                        [this, percent] { session.setDisplayScale(percent); });
    windowMenu->addAction(text("menu.restore_viewers"), &session, &Session::reopen);
    if (!session.valid()) {
        connectButton->setEnabled(false);
        message->setText(text("desktop.core_missing"));
    }
}
void MainWindow::updateCatalog() {
    auto current = windows->currentItem() ? windows->currentItem()->data(Qt::UserRole) : QVariant();
    const auto fingerprint =
        QJsonDocument(session.catalog).toJson(QJsonDocument::Compact) + search->text().toUtf8();
    if (windows->property("catalog").toByteArray() != fingerprint) {
        const auto scroll = windows->verticalScrollBar()->value();
        windows->setProperty("catalog", fingerprint);
        windows->clear();
        for (auto value : session.catalog) {
            auto entry = value.toObject();
            auto title = entry["title"].toString();
            if (title.isEmpty() || !title.contains(search->text(), Qt::CaseInsensitive))
                continue;
            auto *item = new QListWidgetItem(title, windows);
            item->setData(Qt::UserRole, number(entry, "id"));
            if (item->data(Qt::UserRole) == current)
                windows->setCurrentItem(item);
        }
        if (!windows->currentItem() && windows->count())
            windows->setCurrentRow(0);
        windows->verticalScrollBar()->setValue(scroll);
    }
    openButton->setEnabled(session.authenticated && windows->count() > 0);
    auto selected = opened->currentItem() ? opened->currentItem()->data(Qt::UserRole) : QVariant();
    QJsonArray openCatalog;
    for (auto app : session.apps) {
        auto *root = app->viewers.value(app->root);
        openCatalog.append(QJsonObject{{"id", qint64(app->sid)},
                                       {"title", root ? root->windowTitle() : QString("Kokoda")}});
    }
    const auto openFingerprint = QJsonDocument(openCatalog).toJson(QJsonDocument::Compact);
    if (opened->property("catalog").toByteArray() == openFingerprint)
        return;
    opened->setProperty("catalog", openFingerprint);
    opened->clear();
    for (auto app : session.apps) {
        QString title = "Kokoda";
        if (auto *root = app->viewers.value(app->root))
            title = root->windowTitle();
        auto *item = new QListWidgetItem(title, opened);
        item->setData(Qt::UserRole, app->sid);
        if (item->data(Qt::UserRole) == selected)
            opened->setCurrentItem(item);
    }
    if (!opened->currentItem() && opened->count())
        opened->setCurrentRow(0);
}
int runClient(int argc, char **argv) {
    QApplication app(argc, argv);
    app.setOrganizationName("Kokoda");
    app.setApplicationName("Kokoda");
    QCommandLineParser parser;
    parser.addHelpOption();
    parser.addOption({"language", "Interface language: en or zh-CN", "language"});
    parser.process(app);
    if (parser.isSet("language"))
        setLanguage(parser.value("language"));
    else
        setLanguage(QLocale::system().name());
    app.setWindowIcon(QIcon(":/kokoda.png"));
    MainWindow window;
    window.show();
    return app.exec();
}
} // namespace kokoda
