#include "integration.h"
#include <QWidget>
#include <QWindow>
namespace kokoda {
void setNativeOwner(QWidget *window, QWidget *owner, bool independent) {
    window->winId();
    if (owner)
        owner->winId();
    window->windowHandle()->setTransientParent(!independent && owner ? owner->windowHandle()
                                                                     : nullptr);
}
bool constrainNativeResize(QWidget *, void *, QSize, int) {
    return false;
}
} // namespace kokoda
