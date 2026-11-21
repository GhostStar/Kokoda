#include "integration.h"
#include <QWidget>
#include <QWindow>
#define NOMINMAX
#include <windows.h>
namespace kokoda {
void setNativeOwner(QWidget *window, QWidget *owner, bool independent) {
    window->winId();
    if (owner)
        owner->winId();
    window->windowHandle()->setTransientParent(!independent && owner ? owner->windowHandle()
                                                                     : nullptr);
    auto handle = reinterpret_cast<HWND>(window->winId());
    SetWindowLongPtrW(handle, GWLP_HWNDPARENT,
                      !independent && owner ? LONG_PTR(owner->winId()) : 0);
    auto style = GetWindowLongPtrW(handle, GWL_EXSTYLE);
    style = independent ? ((style | WS_EX_APPWINDOW) & ~WS_EX_TOOLWINDOW)
                        : ((style | WS_EX_TOOLWINDOW) & ~WS_EX_APPWINDOW);
    SetWindowLongPtrW(handle, GWL_EXSTYLE, style);
}
bool constrainNativeResize(QWidget *window, void *message, QSize source, int toolbarHeight) {
    const auto *msg = static_cast<MSG *>(message);
    if (msg->message != WM_SIZING || source.isEmpty())
        return false;
    auto *rect = reinterpret_cast<RECT *>(msg->lParam);
    RECT outer{}, client{};
    GetWindowRect(msg->hwnd, &outer);
    GetClientRect(msg->hwnd, &client);
    const auto chromeWidth = (outer.right - outer.left) - (client.right - client.left);
    const auto chromeHeight = (outer.bottom - outer.top) - (client.bottom - client.top) +
                              int(toolbarHeight * window->devicePixelRatioF());
    const double ratio = double(source.width()) / source.height();
    if (msg->wParam == WMSZ_TOP || msg->wParam == WMSZ_BOTTOM) {
        rect->right =
            rect->left + LONG((rect->bottom - rect->top - chromeHeight) * ratio) + chromeWidth;
    } else {
        LONG height = LONG((rect->right - rect->left - chromeWidth) / ratio) + chromeHeight;
        if (msg->wParam == WMSZ_TOPLEFT || msg->wParam == WMSZ_TOPRIGHT)
            rect->top = rect->bottom - height;
        else
            rect->bottom = rect->top + height;
    }
    return true;
}
} // namespace kokoda
