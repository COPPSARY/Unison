import { act, render, screen, waitFor } from "@testing-library/react";
import { strings } from "@/i18n";
import type { YoutubeCommand } from "@/lib/bindings";
import { YouTubeDock } from "./YouTubeDock";

const listeners: Array<(e: { payload: YoutubeCommand }) => void> = [];
const youtubePlayerReport = vi.fn();
const youtubeHostReady = vi.fn();

vi.mock("@/lib/bindings", () => ({
  events: {
    playerYoutubeCommand: {
      listen: (cb: (e: { payload: YoutubeCommand }) => void) => {
        listeners.push(cb);
        return Promise.resolve(() => undefined);
      },
    },
  },
  commands: {
    youtubePlayerReport: (...a: unknown[]) => youtubePlayerReport(...a),
    youtubeHostReady: (...a: unknown[]) => youtubeHostReady(...a),
  },
}));
vi.mock("@/lib/window", () => ({ isTauri: () => true }));

type Events = {
  onReady?: (e: { target: unknown }) => void;
  onStateChange?: (e: { data: number }) => void;
  onError?: (e: { data: number }) => void;
};

const fake = {
  playVideo: vi.fn(),
  pauseVideo: vi.fn(),
  stopVideo: vi.fn(),
  seekTo: vi.fn(),
  setVolume: vi.fn(),
  loadVideoById: vi.fn(),
  getCurrentTime: () => 12.5,
  getDuration: () => 200,
  destroy: vi.fn(),
};
let ytEvents: Events = {};

const send = (cmd: YoutubeCommand) =>
  act(() => {
    listeners.forEach((cb) => cb({ payload: cmd }));
  });

beforeEach(() => {
  listeners.length = 0;
  youtubePlayerReport.mockReset();
  youtubeHostReady.mockReset();
  Object.values(fake).forEach((f) => typeof f === "function" && "mockReset" in f && (f as ReturnType<typeof vi.fn>).mockReset());
  window.YT = {
    Player: vi.fn(function (_el: HTMLIFrameElement, opts: { events?: Events }) {
      ytEvents = opts.events ?? {};
    }) as unknown as NonNullable<typeof window.YT>["Player"],
  };
});
afterEach(() => {
  delete window.YT;
});

describe("YouTubeDock", () => {
  it("registers as the visible host", async () => {
    const { unmount } = render(<YouTubeDock />);
    await waitFor(() => expect(youtubeHostReady).toHaveBeenCalledWith(true));
    unmount();
    expect(youtubeHostReady).toHaveBeenLastCalledWith(false);
  });

  it("loads a video visibly, runs commands and reports state to Rust", async () => {
    render(<YouTubeDock />);
    await waitFor(() => expect(listeners.length).toBeGreaterThan(0));
    expect(screen.queryByTitle(strings.player.youtubeVideo)).not.toBeInTheDocument();

    send({ type: "load", videoId: "abc", startMs: 30_000 });
    const frame = (await screen.findByTitle(strings.player.youtubeVideo)) as HTMLIFrameElement;
    expect(new URL(frame.src).searchParams.get("start")).toBe("30");
    expect(frame.className).toContain("min-h-[200px]");

    await waitFor(() => expect(ytEvents.onReady).toBeDefined());
    act(() => ytEvents.onReady?.({ target: fake }));

    send({ type: "pause" });
    send({ type: "play" });
    send({ type: "seek", positionMs: 61_000 });
    send({ type: "volume", volume: 40 });
    expect(fake.pauseVideo).toHaveBeenCalledOnce();
    expect(fake.playVideo).toHaveBeenCalledOnce();
    expect(fake.seekTo).toHaveBeenCalledWith(61, true);
    expect(fake.setVolume).toHaveBeenCalledWith(40);

    act(() => ytEvents.onStateChange?.({ data: 1 }));
    expect(youtubePlayerReport).toHaveBeenLastCalledWith("playing", 12_500, 200_000, null);
    act(() => ytEvents.onStateChange?.({ data: 0 }));
    expect(youtubePlayerReport).toHaveBeenLastCalledWith("ended", 12_500, 200_000, null);
    act(() => ytEvents.onError?.({ data: 150 }));
    expect(youtubePlayerReport).toHaveBeenLastCalledWith("unstarted", 12_500, 200_000, 150);

    send({ type: "stop" });
    expect(fake.stopVideo).toHaveBeenCalledOnce();
    expect(screen.queryByTitle(strings.player.youtubeVideo)).not.toBeInTheDocument();
  });

  it("ignores the Settings test command", async () => {
    render(<YouTubeDock />);
    await waitFor(() => expect(listeners.length).toBeGreaterThan(0));
    send({ type: "test", videoId: "jNQXAC9IVRw" });
    expect(screen.queryByTitle(strings.player.youtubeVideo)).not.toBeInTheDocument();
  });
});
