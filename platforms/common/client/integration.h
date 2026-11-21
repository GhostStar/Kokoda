#pragma once
#include <QSize>
class QWidget;
namespace kokoda {
// Implemented separately for Windows/X11/Wayland. Coordinates are local logical points.
bool constrainNativeResize(QWidget *window, void *message, QSize source, int toolbarHeight);
void setNativeOwner(QWidget *window, QWidget *owner, bool independent);
} // namespace kokoda
