#pragma once
#include "session.h"
#include <QCheckBox>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMainWindow>
#include <QPushButton>
namespace kokoda {
class MainWindow : public QMainWindow {
    Q_OBJECT
    QLineEdit *host, *password, *search;
    QListWidget *windows, *opened;
    QLabel *message;
    QPushButton *connectButton, *openButton;
    void updateCatalog();

  public:
    Session session;
    explicit MainWindow();
};
int runClient(int argc, char **argv);
} // namespace kokoda
