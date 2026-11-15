import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
import { Dialog } from "./Dialog";
import { DropdownField, type DropdownItem } from "./DropdownField";
import { RefChip } from "./Ref";

const CODES: DropdownItem[] = ["Assistance", "Bug fixed", "Hardware repair", "Other", "Training"].map((l) => ({ value: l.toLowerCase(), label: l }));
const ORGS: DropdownItem[] = ["Demo Org A", "Demo Org B", "Finance and Accounting", "Legal"].map((l, i) => ({ value: String(i), label: l }));

function Controlled({ items, initial = null, searchable = false, onChange = () => {} }: { items: DropdownItem[]; initial?: string | null; searchable?: boolean; onChange?: (v: string) => void }) {
  const [v, setV] = useState<string | null>(initial);
  return (
    <DropdownField
      aria-label="Field"
      items={items}
      value={v}
      searchable={searchable}
      searchPrompt="Search organizations"
      noMatchText="No organization matches"
      onChange={(x) => {
        setV(x);
        onChange(x);
      }}
    />
  );
}

const activeLabel = (combo: HTMLElement) => {
  const id = combo.getAttribute("aria-activedescendant");
  return id ? document.getElementById(id)?.textContent : undefined;
};

describe("DropdownField", () => {
  it("short list: ↓ opens on the current selection, ↓ moves, ↩ picks and closes", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Controlled items={CODES} initial="bug fixed" onChange={onChange} />);
    const combo = screen.getByRole("combobox", { name: "Field" });
    combo.focus();
    await user.keyboard("{ArrowDown}");
    expect(combo).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("listbox")).toBeInTheDocument();
    expect(activeLabel(combo)).toBe("Bug fixed");
    await user.keyboard("{ArrowDown}");
    expect(activeLabel(combo)).toBe("Hardware repair");
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith("hardware repair");
    expect(combo).toHaveAttribute("aria-expanded", "false");
    expect(combo).toHaveTextContent("Hardware repair");
  });

  it("short list: a letter jumps to the first item starting with it", async () => {
    const user = userEvent.setup();
    render(<Controlled items={CODES} />);
    const combo = screen.getByRole("combobox", { name: "Field" });
    combo.focus();
    await user.keyboard("t");
    expect(combo).toHaveAttribute("aria-expanded", "true");
    expect(activeLabel(combo)).toBe("Training");
  });

  it("Esc closes an open list without reaching the sheet", async () => {
    const user = userEvent.setup();
    const parentKey = vi.fn();
    render(
      <div onKeyDown={(e) => e.key === "Escape" && parentKey()}>
        <Controlled items={CODES} />
      </div>,
    );
    const combo = screen.getByRole("combobox", { name: "Field" });
    combo.focus();
    await user.keyboard("{ArrowDown}");
    await user.keyboard("{Escape}");
    expect(combo).toHaveAttribute("aria-expanded", "false");
    expect(parentKey).not.toHaveBeenCalled();
    // a second Esc (list closed) is the sheet's
    await user.keyboard("{Escape}");
    expect(parentKey).toHaveBeenCalledTimes(1);
  });

  it("searchable: typing filters by every word, highlights the first match, ↩ picks", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Controlled items={ORGS} searchable onChange={onChange} />);
    const combo = screen.getByRole("combobox", { name: "Field" });
    await user.click(combo);
    await user.keyboard("org b");
    const options = screen.getAllByRole("option");
    expect(options.map((o) => o.textContent)).toEqual(["Demo Org B"]);
    expect(activeLabel(combo)).toBe("Demo Org B");
    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenCalledWith("1");
    expect(combo).toHaveValue("Demo Org B");
  });

  it("searchable: no match says so in words", async () => {
    const user = userEvent.setup();
    render(<Controlled items={ORGS} searchable />);
    const combo = screen.getByRole("combobox", { name: "Field" });
    await user.click(combo);
    await user.keyboard("zzz");
    expect(screen.getByText("No organization matches")).toBeInTheDocument();
  });

  it("can't open when disabled or empty", async () => {
    const user = userEvent.setup();
    render(<DropdownField aria-label="Empty" items={[]} value={null} onChange={() => {}} placeholder="Choose a team first" />);
    const combo = screen.getByRole("combobox", { name: "Empty" });
    combo.focus();
    await user.keyboard("{ArrowDown}");
    expect(combo).toHaveAttribute("aria-expanded", "false");
    expect(combo).toHaveTextContent("Choose a team first");
  });
});

describe("Dialog", () => {
  function Sheet({ onCancel = () => {}, onConfirm = () => {}, confirmDisabled = false }: { onCancel?: () => void; onConfirm?: () => void; confirmDisabled?: boolean }) {
    return (
      <Dialog open title="Set R-000412 to pending" onCancel={onCancel} onConfirm={onConfirm} confirmLabel="Set to pending" confirmDisabled={confirmDisabled}>
        <label>
          Reason
          <textarea aria-label="Reason" />
        </label>
        <Controlled items={CODES} />
      </Dialog>
    );
  }

  it("is a labelled modal dialog and focuses the first field", async () => {
    render(<Sheet />);
    const dialog = screen.getByRole("dialog", { name: "Set R-000412 to pending" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    await waitFor(() => expect(screen.getByLabelText("Reason")).toHaveFocus());
  });

  it("traps Tab inside", async () => {
    const user = userEvent.setup();
    render(
      <>
        <button>outside</button>
        <Sheet />
      </>,
    );
    await waitFor(() => expect(screen.getByLabelText("Reason")).toHaveFocus());
    const confirm = screen.getByRole("button", { name: "Set to pending" });
    confirm.focus();
    await user.tab();
    expect(screen.getByLabelText("Reason")).toHaveFocus();
    await user.tab({ shift: true });
    expect(confirm).toHaveFocus();
    expect(screen.getByRole("button", { name: "outside" })).not.toHaveFocus();
  });

  it("Esc cancels, ⌘↩ confirms", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    const onConfirm = vi.fn();
    render(<Sheet onCancel={onCancel} onConfirm={onConfirm} />);
    await waitFor(() => expect(screen.getByLabelText("Reason")).toHaveFocus());
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onConfirm).toHaveBeenCalledTimes(1);
    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("⌘↩ does nothing while the confirm button is disabled", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    render(<Sheet onConfirm={onConfirm} confirmDisabled />);
    await waitFor(() => expect(screen.getByLabelText("Reason")).toHaveFocus());
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("Esc with a dropdown open closes only the dropdown", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<Sheet onCancel={onCancel} />);
    // let the sheet's initial focus (next frame) land first, or it steals focus from the open list
    await waitFor(() => expect(screen.getByLabelText("Reason")).toHaveFocus());
    const combo = screen.getByRole("combobox", { name: "Field" });
    combo.focus();
    await user.keyboard("{ArrowDown}");
    expect(combo).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");
    expect(combo).toHaveAttribute("aria-expanded", "false");
    expect(onCancel).not.toHaveBeenCalled();
  });
});

describe("Composer", () => {
  function Box({ onSend, sendDisabled = false }: { onSend: () => void; sendDisabled?: boolean }) {
    const [v, setV] = useState("");
    return <Composer value={v} onChange={setV} onSend={onSend} label="Public entry" sendLabel="Add to public log" sendDisabled={sendDisabled} />;
  }

  it("⌘↩ sends; plain Return is a new line", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<Box onSend={onSend} />);
    const box = screen.getByRole("textbox", { name: "Public entry" });
    await user.click(box);
    await user.keyboard("line one{Enter}line two");
    expect(onSend).not.toHaveBeenCalled();
    expect(box).toHaveValue("line one\nline two");
    await user.keyboard("{Meta>}{Enter}{/Meta}");
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("⌘↩ does nothing when sending isn't allowed", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<Box onSend={onSend} sendDisabled />);
    await user.click(screen.getByRole("textbox", { name: "Public entry" }));
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(onSend).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: /Add to public log/ })).toBeDisabled();
  });

  it("hands dropped files to the caller", () => {
    const onDropFiles = vi.fn();
    const { container } = render(
      <Composer value="" onChange={() => {}} onSend={() => {}} label="Public entry" sendLabel="Add" onDropFiles={onDropFiles} />,
    );
    const file = new File(["x"], "shot.png", { type: "image/png" });
    fireEvent.drop(container.firstElementChild!, { dataTransfer: { types: ["Files"], files: [file] } });
    expect(onDropFiles).toHaveBeenCalledWith([file], expect.anything());
  });
});

describe("RefChip", () => {
  it("copies the ref and reads Copied for 2 s", async () => {
    vi.useFakeTimers();
    try {
      const onCopy = vi.fn();
      render(<RefChip value="R-000412" onCopy={onCopy} />);
      const button = screen.getByRole("button", { name: "R-000412, copy ref" });
      await act(async () => {
        fireEvent.click(button);
      });
      expect(onCopy).toHaveBeenCalledWith("R-000412");
      expect(screen.getByRole("status")).toHaveTextContent("Copied R-000412");
      await act(async () => {
        vi.advanceTimersByTime(2000);
      });
      expect(screen.getByRole("status")).toHaveTextContent("");
    } finally {
      vi.useRealTimers();
    }
  });

  it("stays quiet when copying fails", async () => {
    const onCopy = vi.fn().mockRejectedValue(new Error("denied"));
    render(<RefChip value="I-000088" onCopy={onCopy} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "I-000088, copy ref" }));
    });
    expect(screen.getByRole("status")).toHaveTextContent("");
  });
});
