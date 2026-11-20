import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Play } from "lucide-react";
import { useToastStore } from "@/stores/toast";
import {
  Banner,
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogTrigger,
  Icon,
  IconButton,
  Input,
  Menu,
  MenuContent,
  MenuItem,
  MenuTrigger,
  Segmented,
  Skeleton,
  Toaster,
} from ".";

describe("Button", () => {
  it("uses the accent only for the primary variant", () => {
    render(
      <>
        <Button variant="primary">A</Button>
        <Button variant="secondary">B</Button>
      </>,
    );
    expect(screen.getByRole("button", { name: "A" }).className).toContain("bg-accent");
    expect(screen.getByRole("button", { name: "B" }).className).not.toContain("bg-accent");
  });

  it("does not fire when disabled", async () => {
    const onClick = vi.fn();
    render(
      <Button disabled onClick={onClick}>
        X
      </Button>,
    );
    await userEvent.click(screen.getByRole("button", { name: "X" }));
    expect(onClick).not.toHaveBeenCalled();
  });

  it("is a capsule with spring press feedback", () => {
    render(<Button>A</Button>);
    expect(screen.getByRole("button").className).toMatch(/rounded-full/);
    expect(screen.getByRole("button").className).toMatch(/u-press/);
  });

  it("never translates or shadows on hover", () => {
    render(<Button variant="primary">A</Button>);
    expect(screen.getByRole("button").className).not.toMatch(/hover:(translate|scale|shadow)/);
  });
});

describe("IconButton", () => {
  it("has an accessible name and reports pressed state", () => {
    render(<IconButton label="Repeat" active icon={<Icon as={Play} />} />);
    const btn = screen.getByRole("button", { name: "Repeat" });
    expect(btn).toHaveAttribute("aria-pressed", "true");
  });
});

describe("Input", () => {
  it("marks invalid state for assistive tech", () => {
    render(<Input invalid aria-label="Client ID" />);
    expect(screen.getByRole("textbox", { name: "Client ID" })).toHaveAttribute(
      "aria-invalid",
      "true",
    );
  });
});

describe("Segmented", () => {
  const options = [
    { value: "all" as const, label: "All" },
    { value: "local" as const, label: "This PC" },
  ];

  it("emits the new value and never deselects", async () => {
    const onChange = vi.fn();
    render(<Segmented label="Source" value="all" onValueChange={onChange} options={options} />);
    await userEvent.click(screen.getByRole("radio", { name: "This PC" }));
    expect(onChange).toHaveBeenCalledWith("local");
    onChange.mockClear();
    await userEvent.click(screen.getByRole("radio", { name: "All" }));
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe("Menu", () => {
  it("opens and runs the selected item", async () => {
    const onSelect = vi.fn();
    render(
      <Menu>
        <MenuTrigger asChild>
          <Button>Open</Button>
        </MenuTrigger>
        <MenuContent>
          <MenuItem onSelect={onSelect}>Add to queue</MenuItem>
          <MenuItem disabled>Disabled item</MenuItem>
        </MenuContent>
      </Menu>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Add to queue" }));
    expect(onSelect).toHaveBeenCalledOnce();
  });
});

describe("Dialog", () => {
  it("opens from its trigger and closes with Cancel or Escape", async () => {
    render(
      <Dialog>
        <DialogTrigger asChild>
          <Button>Open</Button>
        </DialogTrigger>
        <DialogContent
          title="Remove Mock Stream?"
          description="Its songs stay in your library."
          footer={
            <DialogClose asChild>
              <Button>Cancel</Button>
            </DialogClose>
          }
        />
      </Dialog>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(await screen.findByRole("dialog", { name: "Remove Mock Stream?" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    await screen.findByRole("dialog");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

describe("Banner", () => {
  it("announces problems as alerts and info as status", () => {
    render(
      <>
        <Banner tone="danger">Expired</Banner>
        <Banner>Offline</Banner>
      </>,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Expired");
    expect(screen.getByRole("status")).toHaveTextContent("Offline");
  });
});

describe("Skeleton", () => {
  it("is hidden from assistive tech", () => {
    const { container } = render(<Skeleton />);
    expect(container.firstChild).toHaveAttribute("aria-hidden", "true");
  });
});

describe("Toaster", () => {
  beforeEach(() => useToastStore.setState({ toasts: [] }));

  it("shows pushed toasts with their action", async () => {
    const onUndo = vi.fn();
    render(<Toaster />);
    act(() => {
      useToastStore.getState().push({
        title: "Added to Late night",
        action: { label: "Undo", onAction: onUndo },
      });
    });
    expect(await screen.findByText("Added to Late night")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(onUndo).toHaveBeenCalledOnce();
  });

  it("keeps at most three toasts", () => {
    const { push } = useToastStore.getState();
    for (let i = 0; i < 5; i++) push({ title: `t${i}` });
    expect(useToastStore.getState().toasts.map((t) => t.title)).toEqual(["t2", "t3", "t4"]);
  });
});
