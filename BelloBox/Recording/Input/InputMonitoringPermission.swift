import ApplicationServices
import Foundation

enum InputMonitoringPermission {
    static func status() -> PermissionStatus {
        CGPreflightListenEventAccess() ? .granted : .notDetermined
    }

    static func request() -> PermissionStatus {
        CGRequestListenEventAccess() ? .granted : .denied
    }
}
