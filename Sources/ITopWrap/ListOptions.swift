import Foundation
import ITopKit
import Observation
import SwiftUI

/// The date range and sort shared by every ticket list, remembered between launches.
@MainActor @Observable
final class ListOptions {
    enum Range: Int, CaseIterable, Identifiable {
        case any = 0, week = 7, month = 30, quarter = 90

        var id: Int { rawValue }
        var title: String {
            switch self {
            case .any: "Any time"
            case .week: "Last 7 days"
            case .month: "Last 30 days"
            case .quarter: "Last 90 days"
            }
        }
    }

    private let defaults: UserDefaults

    var range: Range { didSet { defaults.set(range.rawValue, forKey: "list.range") } }
    var includeClosed: Bool {
        didSet {
            defaults.set(includeClosed, forKey: "list.includeClosed")
            // Resolved and closed always come with a date range (TicketScope).
            if includeClosed && range == .any { range = .month }
        }
    }
    var sort: TicketSort { didSet { defaults.set(sort.rawValue, forKey: "list.sort") } }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        range = Range(rawValue: defaults.integer(forKey: "list.range")) ?? .any
        includeClosed = defaults.bool(forKey: "list.includeClosed")
        sort = TicketSort(rawValue: defaults.string(forKey: "list.sort") ?? "") ?? .priority
        if includeClosed && range == .any { range = .month }
    }

    var scope: TicketScope {
        TicketScope(includeClosed: includeClosed, openedWithinDays: range == .any ? nil : range.rawValue)
    }

    /// "Open tickets", "Open, last 30 days", "All, last 30 days".
    var rangeLabel: String {
        switch (includeClosed, range) {
        case (false, .any): "Open tickets"
        case (false, let r): "Open, \(r.title.lowercased())"
        case (true, let r): "All, \(r.title.lowercased())"
        }
    }
}

/// The range and sort menus above a ticket list. They're two separate children, so the caller's
/// container decides the layout: a wrapping row in the narrow list column, an HStack elsewhere.
struct ListOptionsMenus: View {
    @Bindable var options: ListOptions

    var body: some View {
        Group {
            Menu {
                Picker("Opened", selection: $options.range) {
                    ForEach(ListOptions.Range.allCases) { r in
                        Text(r.title).tag(r).disabled(r == .any && options.includeClosed)
                    }
                }
                .pickerStyle(.inline)
                Divider()
                Toggle("Include resolved and closed", isOn: $options.includeClosed)
            } label: {
                chip(options.rangeLabel, symbol: "calendar", on: options.scope != .open)
            }
            .help("Which tickets to show")
            .accessibilityLabel("Date range: \(options.rangeLabel)")

            Menu {
                Picker("Sort", selection: $options.sort) {
                    ForEach(TicketSort.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.inline)
            } label: {
                chip(options.sort == .priority ? "Sort" : options.sort.title, symbol: "arrow.up.arrow.down", on: options.sort != .priority)
            }
            .help("Sort inside each status group: \(options.sort.title)")
            .accessibilityLabel("Sort: \(options.sort.title)")
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.visible)
        .fixedSize()
    }

    private func chip(_ text: String, symbol: String, on: Bool) -> some View {
        Label(text, systemImage: symbol)
            .font(.system(size: 12))
            .lineLimit(1)
            .padding(.horizontal, 9).frame(height: 24)
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(on ? Theme.text : Theme.separator))
    }
}
