import AppKit
import Testing
@testable import ITopWrap

@Suite("The menu bar time slides out of the icon, to its right")
@MainActor
struct MenuBarSlideTests {
    let icon = MenuBarIcon.image.size.width

    @Test("No title: just the icon, and no room for text")
    func iconOnly() {
        #expect(MenuBarIcon.textWidth("") == 0)
        #expect(MenuBarIcon.label("").size.width == icon)
    }

    @Test("Fully shown: the icon plus the gap and the text")
    func full() {
        let text = MenuBarIcon.textWidth("00:12:40")
        #expect(text > 4)
        #expect(MenuBarIcon.label("00:12:40").size.width == icon + text)
    }

    @Test("Mid-slide: only part of the text is shown, the icon keeps its width")
    func partial() {
        let text = MenuBarIcon.textWidth("00:12:40")
        #expect(MenuBarIcon.label("00:12:40", visible: text / 2).size.width == icon + text / 2)
        #expect(MenuBarIcon.label("00:12:40", visible: 0).size.width == icon)
    }

    @Test("Each step of the slide eases out and ends exactly on the target")
    func steps() {
        let s = MenuBarSlide.steps(from: 0, to: 60, count: 12)
        #expect(s.count == 12)
        #expect(s.last == 60)
        #expect(zip(s, s.dropFirst()).allSatisfy { $0 <= $1 })
        #expect(s[0] > 60 / 12, "eases out: the first step is bigger than an even one")
        #expect(MenuBarSlide.steps(from: 60, to: 0, count: 12).last == 0)
    }
}
