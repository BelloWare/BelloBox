import Combine
import Foundation

/// Lets a window controller steer an already open window to a page, such as
/// a Settings category or a Home category chosen in the palette.
final class WindowNavigation<Destination: Equatable>: ObservableObject {
    @Published var requested: Destination?
    init(_ requested: Destination? = nil) { self.requested = requested }
}
