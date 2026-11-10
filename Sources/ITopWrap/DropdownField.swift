import SwiftUI

// The one picker used across the app (DESIGN.md: "Dropdowns (one component everywhere)").
//
// - Look: a 32 pt field with a chevron; the list opens below it with an 8 pt radius and a
//   hairline border, a check mark on the selected item and a grey highlight on the keyboard
//   position. The field border darkens while open, and the focus ring wraps the whole field.
// - Search only where lists get long: a field with `searchPrompt` shows a magnifier and you type
//   into it ("Search organizations"); short lists open without a search field.
// - Keyboard: ↓/↑ opens the list and moves, ↩ picks, Esc closes, and typing filters (or, in a
//   short list, jumps to the first item starting with that letter).
// - One open at a time: the sheet's `dropdownHost()` keeps which one is open and draws its list
//   above everything else, so the field keeps keyboard focus (a popover would take it).

/// Which dropdown is open in a sheet, shared through the environment.
@Observable final class DropdownState {
    var openID: UUID?
}

private struct DropdownStateKey: EnvironmentKey {
    static var defaultValue: DropdownState? { nil }
}

extension EnvironmentValues {
    var dropdownState: DropdownState? {
        get { self[DropdownStateKey.self] }
        set { self[DropdownStateKey.self] = newValue }
    }
}

/// The open dropdown's list, passed up to the host to draw on top.
private struct DropdownListKey: PreferenceKey {
    static var defaultValue: DropdownListValue? { nil }
    static func reduce(value: inout DropdownListValue?, nextValue: () -> DropdownListValue?) {
        value = nextValue() ?? value
    }
}

private struct DropdownListValue {
    let anchor: Anchor<CGRect>
    let height: CGFloat
    let list: AnyView
}

extension View {
    /// Hosts the app's dropdowns in a sheet: one open at a time, lists drawn above the fields,
    /// a click outside closes the open one.
    func dropdownHost() -> some View { modifier(DropdownHost()) }

    /// Escape closes an open dropdown first; only with none open does it cancel the sheet.
    func cancelShortcutUnlessDropdownOpen() -> some View { modifier(CancelShortcut()) }
}

private struct DropdownHost: ViewModifier {
    @State private var state = DropdownState()

    func body(content: Content) -> some View {
        content
            .environment(\.dropdownState, state)
            .overlayPreferenceValue(DropdownListKey.self) { value in
                GeometryReader { proxy in
                    if let value, state.openID != nil {
                        let field = proxy[value.anchor]
                        // Below the field when it fits, otherwise above it.
                        let below = field.maxY + 4 + value.height <= proxy.size.height
                        ZStack(alignment: .topLeading) {
                            Color.black.opacity(0.001)
                                .onTapGesture { state.openID = nil }
                                .accessibilityHidden(true)
                            value.list
                                .frame(width: field.width)
                                .offset(x: field.minX, y: below ? field.maxY + 4 : field.minY - 4 - value.height)
                        }
                    }
                }
            }
    }
}

private struct CancelShortcut: ViewModifier {
    @Environment(\.dropdownState) private var state

    func body(content: Content) -> some View {
        content.keyboardShortcut(state?.openID == nil ? .cancelAction : nil)
    }
}

struct DropdownField<Item: Identifiable & Hashable>: View {
    /// The field's name for VoiceOver ("Organization", "Impact").
    let name: String
    let placeholder: String
    let items: [Item]
    @Binding var selection: Item?
    let label: (Item) -> String
    /// Set for long lists: the field takes typing, with this prompt ("Search organizations").
    var searchPrompt: String?
    /// What an empty search says ("No caller in Org A matches").
    var noMatch: String?

    init(name: String, placeholder: String, items: [Item], selection: Binding<Item?>, label: @escaping (Item) -> String,
         searchPrompt: String? = nil, noMatch: String? = nil) {
        self.name = name
        self.placeholder = placeholder
        self.items = items
        _selection = selection
        self.label = label
        self.searchPrompt = searchPrompt
        self.noMatch = noMatch
    }

    init(name: String, placeholder: String, items: [Item], selection: Binding<Item?>, label: KeyPath<Item, String>,
         searchPrompt: String? = nil, noMatch: String? = nil) {
        self.init(name: name, placeholder: placeholder, items: items, selection: selection, label: { $0[keyPath: label] },
                  searchPrompt: searchPrompt, noMatch: noMatch)
    }

    @Environment(\.dropdownState) private var state
    @Environment(\.isEnabled) private var isEnabled
    @State private var id = UUID()
    @State private var query = ""
    @State private var editing = false
    @State private var highlight = 0
    @FocusState private var focused: Bool

    private static var rowHeight: CGFloat { 26 }
    private var searchable: Bool { searchPrompt != nil }
    private var isOpen: Bool { state?.openID == id }
    private var usable: Bool { isEnabled && !items.isEmpty }

    private var matches: [Item] {
        guard searchable, editing else { return items }
        let words = query.lowercased().split(separator: " ").map(String.init)
        guard !words.isEmpty else { return items }
        return items.filter { item in let l = label(item).lowercased(); return words.allSatisfy { l.contains($0) } }
    }

    var body: some View {
        field
            .frame(height: 32)
            .background(Theme.chip.opacity(isEnabled ? 0.6 : 0.3), in: RoundedRectangle(cornerRadius: 7))
            .overlay(RoundedRectangle(cornerRadius: 7)
                .stroke(isOpen || focused ? Theme.text.opacity(0.55) : Theme.separator, lineWidth: focused ? 1.5 : 1))
            .contentShape(RoundedRectangle(cornerRadius: 7))
            .anchorPreference(key: DropdownListKey.self, value: .bounds) { anchor in
                isOpen ? DropdownListValue(anchor: anchor, height: listHeight, list: AnyView(list)) : nil
            }
            .onChange(of: focused) { _, now in if !now { close() } }
            .onChange(of: state?.openID) { _, open in if open != id { editing = false; query = "" } }
            .modifier(ShortListAccessibility(enabled: !searchable, label: name, value: spokenValue.isEmpty ? placeholder : spokenValue, usable: usable,
                                             action: { focused = true; toggle() }))
    }

    /// The selection, or while the list is open, the highlighted item.
    private var spokenValue: String {
        if isOpen, matches.indices.contains(highlight) { return label(matches[highlight]) }
        return selection.map(label) ?? ""
    }

    // MARK: Field

    @ViewBuilder
    private var field: some View {
        HStack(spacing: 6) {
            if searchable {
                Image(systemName: "magnifyingglass").font(.system(size: 11)).foregroundStyle(Theme.secondary)
                TextField("", text: Binding(
                    get: { editing ? query : (selection.map(label) ?? "") },
                    set: { new in
                        // Showing the picked name writes it back here; that isn't typing.
                        if !editing && new == (selection.map(label) ?? "") { return }
                        query = new
                        editing = true
                        highlight = 0
                        open()
                    }),
                    prompt: Text(usable ? (searchPrompt ?? placeholder) : placeholder).foregroundStyle(Theme.secondary))
                    .textFieldStyle(.plain)
                    .focused($focused)
                    // The text field is the control VoiceOver reaches, so you can type into it.
                    .accessibilityLabel(name)
                    .accessibilityValue(spokenValue.isEmpty ? (usable ? (searchPrompt ?? placeholder) : placeholder) : spokenValue)
                    .accessibilityHint(usable ? "Type to filter, then use the up and down arrow keys and Return to pick" : "")
                    .disabled(!usable)
                    .onSubmit { pick() }
                    .onKeyPress(.downArrow) { move(1) }
                    .onKeyPress(.upArrow) { move(-1) }
                    .onKeyPress(.escape) { escape() }
            } else {
                Text(selection.map(label) ?? placeholder)
                    .foregroundStyle(selection == nil ? Theme.secondary : Theme.text)
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            Image(systemName: "chevron.up.chevron.down").font(.system(size: 10)).foregroundStyle(Theme.secondary)
        }
        .padding(.horizontal, 10)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .contentShape(Rectangle())
        .onTapGesture {
            guard usable else { return }
            focused = true
            toggle()
        }
        .modifier(ShortListKeys(enabled: !searchable && usable, focused: $focused,
                                move: move, pick: { pick(); return .handled }, escape: escape, jump: jump))
    }

    // MARK: List

    private var listHeight: CGFloat {
        min(260, CGFloat(max(matches.count, 1)) * Self.rowHeight + 8)
    }

    private var list: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(matches.enumerated()), id: \.element.id) { index, item in
                        HStack {
                            Text(label(item)).lineLimit(1)
                            Spacer()
                            if item == selection { Image(systemName: "checkmark").font(.system(size: 11, weight: .semibold)) }
                        }
                        .padding(.horizontal, 10)
                        .frame(height: Self.rowHeight)
                        .background(index == highlight ? Theme.chip : Color.clear, in: RoundedRectangle(cornerRadius: 5))
                        .contentShape(Rectangle())
                        .onTapGesture { pick(item) }
                        .onHover { if $0 { highlight = index } }
                        .id(index)
                    }
                    if matches.isEmpty {
                        Text(noMatch ?? "Nothing matches “\(query)”.")
                            .foregroundStyle(Theme.secondary)
                            .padding(.horizontal, 10)
                            .frame(height: Self.rowHeight)
                    }
                }
                .padding(4)
            }
            .onChange(of: highlight) { _, new in proxy.scrollTo(new) }
        }
        .frame(height: listHeight)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(Theme.separator, lineWidth: 0.5))
        .shadow(color: .black.opacity(0.18), radius: 8, y: 3)
        .font(.system(size: 13))
    }

    // MARK: Actions

    private func open() {
        guard usable else { return }
        if !isOpen {
            state?.openID = id
            highlight = max(0, matches.firstIndex { $0 == selection } ?? 0)
        }
    }

    private func close() {
        if isOpen { state?.openID = nil }
        editing = false
        query = ""
    }

    private func toggle() {
        if isOpen { close() } else { open() }
    }

    private func move(_ step: Int) -> KeyPress.Result {
        guard usable else { return .ignored }
        if !isOpen { open(); return .handled }
        let count = matches.count
        guard count > 0 else { return .handled }
        highlight = min(max(0, highlight + step), count - 1)
        AccessibilityNotification.Announcement(label(matches[highlight])).post()
        return .handled
    }

    private func pick(_ item: Item? = nil) {
        if let item {
            selection = item
        } else if isOpen, matches.indices.contains(highlight) {
            selection = matches[highlight]
        } else if !isOpen {
            open()
            return
        }
        close()
    }

    private func escape() -> KeyPress.Result {
        guard isOpen || editing else { return .ignored }
        close()
        return .handled
    }

    /// Short lists: a letter jumps to the first item starting with it.
    private func jump(_ character: Character) -> KeyPress.Result {
        guard let i = items.firstIndex(where: { label($0).lowercased().hasPrefix(String(character).lowercased()) }) else { return .ignored }
        open()
        highlight = i
        AccessibilityNotification.Announcement(label(items[i])).post()
        return .handled
    }
}

/// Short lists are one control for VoiceOver; search fields keep their text field reachable.
private struct ShortListAccessibility: ViewModifier {
    let enabled: Bool
    let label: String
    let value: String
    let usable: Bool
    let action: () -> Void

    func body(content: Content) -> some View {
        if enabled {
            content
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(label)
                .accessibilityValue(value)
                .accessibilityHint(usable ? "Use the up and down arrow keys to choose, Return to pick" : "")
                .accessibilityAddTraits(.isButton)
                .accessibilityAction { action() }
        } else {
            content.accessibilityElement(children: .contain)
        }
    }
}

/// Keyboard for short lists, where the field itself takes focus (there's no text field).
private struct ShortListKeys: ViewModifier {
    let enabled: Bool
    var focused: FocusState<Bool>.Binding
    let move: (Int) -> KeyPress.Result
    let pick: () -> KeyPress.Result
    let escape: () -> KeyPress.Result
    let jump: (Character) -> KeyPress.Result

    func body(content: Content) -> some View {
        if enabled {
            content
                .focusable()
                .focusEffectDisabled()
                .focused(focused)
                .onKeyPress(.downArrow) { move(1) }
                .onKeyPress(.upArrow) { move(-1) }
                .onKeyPress(.return) { pick() }
                .onKeyPress(.space) { pick() }
                .onKeyPress(.escape) { escape() }
                .onKeyPress(characters: .letters) { press in press.characters.first.map(jump) ?? .ignored }
        } else {
            content
        }
    }
}
