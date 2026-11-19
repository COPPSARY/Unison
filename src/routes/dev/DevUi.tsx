import {
  AlertTriangle,
  CloudOff,
  Heart,
  ListPlus,
  MoreHorizontal,
  Pause,
  Play,
  Repeat,
  Search,
  Shuffle,
  SkipBack,
  SkipForward,
  Trash2,
  Unplug,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Link, useSearchParams } from "react-router";
import {
  Banner,
  Button,
  Dialog,
  DialogClose,
  DialogContent,
  DialogTrigger,
  Drawer,
  DrawerContent,
  DrawerTrigger,
  Icon,
  IconButton,
  Input,
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
  Segmented,
  Select,
  Skeleton,
  Tooltip,
} from "@/components/ui";
import { format, strings } from "@/i18n";
import type { Rgb } from "@/lib/accent";
import { cn } from "@/lib/cn";
import { toast } from "@/stores/toast";
import { useUiStore, type Theme } from "@/stores/ui";

const s = strings.devUi;

const TOKENS = [
  "accent",
  "background",
  "surface",
  "surface-raised",
  "text",
  "text-muted",
  "text-tertiary",
  "text-disabled",
  "fill",
  "fill-secondary",
  "fill-tertiary",
  "fill-quaternary",
  "border",
  "border-strong",
  "success",
  "warning",
  "danger",
] as const;

/** Sample artwork colours (stand-ins for real covers until playback lands). */
const PRESETS: Array<{ label: string; rgb: Rgb | null }> = [
  { label: s.noArtwork, rgb: null },
  { label: s.presetCrimson, rgb: { r: 196, g: 32, b: 54 } },
  { label: s.presetTeal, rgb: { r: 16, g: 120, b: 128 } },
  { label: s.presetAmber, rgb: { r: 232, g: 150, b: 40 } },
  { label: s.presetBlue, rgb: { r: 40, g: 92, b: 220 } },
  { label: s.presetGrey, rgb: { r: 120, g: 120, b: 120 } },
];

const REDIRECT_URIS = [43821, 43822, 43823].map((p) => `http://127.0.0.1:${p}/callback`);

type Source = "all" | "spotify" | "youtube";

const rgb = (c: Rgb, a = 1) => `rgb(${c.r} ${c.g} ${c.b} / ${a})`;

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="py-6">
      <h2 className="mb-4 text-title3 text-text">{title}</h2>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
    </section>
  );
}

function Labeled({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-footnote text-text-muted">{label}</span>
      {children}
    </div>
  );
}

/** Stand-in "album art" so glass has something to refract. Dev only. */
function ArtworkStage({ color, children }: { color: Rgb; children: ReactNode }) {
  const second = { r: 255 - color.g, g: color.b, b: color.r };
  return (
    <div className="relative h-80 w-full overflow-hidden rounded-panel bg-surface">
      <div
        aria-hidden="true"
        className="absolute inset-0"
        style={{
          background: [
            `radial-gradient(circle at 20% 30%, ${rgb(color)} 0 18%, transparent 19%)`,
            `radial-gradient(circle at 70% 60%, ${rgb(second)} 0 22%, transparent 23%)`,
            `radial-gradient(circle at 45% 85%, ${rgb(color, 0.7)} 0 12%, transparent 13%)`,
            "repeating-linear-gradient(45deg, rgb(255 255 255 / 0.06) 0 12px, transparent 12px 24px)",
          ].join(", "),
        }}
      />
      <div className="relative flex h-full flex-col justify-between p-5">{children}</div>
    </div>
  );
}

/** Storybook-style gallery: every UI kit component in every state (T-03 / T-03a). */
export function DevUi() {
  const theme = useUiStore((st) => st.theme);
  const setTheme = useUiStore((st) => st.setTheme);
  const accentSource = useUiStore((st) => st.accentSource);
  const setAccentSource = useUiStore((st) => st.setAccentSource);
  const frost = useUiStore((st) => st.glassFrost);
  const setFrost = useUiStore((st) => st.setGlassFrost);
  const [source, setSource] = useState<Source>("all");
  const [playing, setPlaying] = useState(false);
  const [liked, setLiked] = useState(true);
  const [params] = useSearchParams();

  // Dev convenience for screenshots: /dev/ui?theme=light&accent=2 (preset index).
  useEffect(() => {
    const t = params.get("theme");
    if (t === "light" || t === "dark") setTheme(t);
    const preset = PRESETS[Number(params.get("accent") ?? -1)];
    if (preset) setAccentSource(preset.rgb);
  }, [params, setTheme, setAccentSource]);

  const stageColor = accentSource ?? { r: 139, g: 112, b: 255 };
  const sourceOptions: Array<{ value: Source; label: string }> = [
    { value: "all", label: strings.source.all },
    { value: "spotify", label: strings.source.spotify },
    { value: "youtube", label: strings.source.youtube },
  ];
  const sortOptions = [
    { value: "added", label: strings.sort.dateAdded },
    { value: "title", label: strings.sort.title },
    { value: "artist", label: strings.sort.artist },
  ];

  return (
    <div className="min-h-full bg-background">
      <header className="glass glass-flat sticky top-0 z-30 flex h-14 items-center justify-between gap-4 px-8">
        <h1 className="text-title2 text-text">{s.title}</h1>
        <div className="flex items-center gap-3">
          <Segmented<Theme>
            label={s.theme}
            value={theme}
            onValueChange={setTheme}
            options={[
              { value: "dark", label: s.dark },
              { value: "light", label: s.light },
            ]}
          />
          <Link to="/" className="u-tone text-callout text-text-muted hover:text-text">
            {s.back}
          </Link>
        </div>
      </header>

      <div className="mx-auto max-w-[960px] px-8 pb-16">
        <Section title={s.materials}>
          <ArtworkStage color={stageColor}>
            <div className="flex flex-wrap gap-3">
              {(
                [
                  ["glass", s.regular],
                  ["glass-thick", s.thick],
                  ["glass-clear", s.clear],
                ] as const
              ).map(([cls, label]) => (
                <div
                  key={cls}
                  className={cn(cls, "flex h-20 w-40 items-end rounded-panel p-3 text-headline")}
                >
                  {label}
                </div>
              ))}
              <div className="glass-clear glass-lens flex h-20 w-40 items-end rounded-full p-3 px-5 text-headline">
                {s.lens}
              </div>
            </div>
            {/* Mock player bar: the chrome layer floating over content. */}
            <div className="glass flex items-center gap-3 rounded-full p-2 pr-4">
              <div
                className="size-10 shrink-0 rounded-full"
                style={{ background: rgb(stageColor) }}
                aria-hidden="true"
              />
              <div className="min-w-0 flex-1">
                <p className="truncate text-headline text-text">{s.sampleTitle}</p>
                <p className="truncate text-callout text-text-muted">{s.sampleArtist}</p>
              </div>
              <IconButton label={strings.player.previous} icon={<Icon as={SkipBack} size={20} />} />
              <IconButton
                label={playing ? strings.player.pause : strings.player.play}
                variant="primary"
                size="lg"
                icon={<Icon as={playing ? Pause : Play} size={20} fill="currentColor" />}
                onClick={() => setPlaying((p) => !p)}
              />
              <IconButton label={strings.player.next} icon={<Icon as={SkipForward} size={20} />} />
            </div>
          </ArtworkStage>
          <div className="flex w-full flex-wrap items-end gap-6">
            <Labeled label={s.accent}>
              <div className="flex flex-wrap gap-2">
                {PRESETS.map((p) => (
                  <Tooltip key={p.label} content={p.label}>
                    <button
                      type="button"
                      aria-label={p.label}
                      aria-pressed={accentSource === p.rgb}
                      onClick={() => setAccentSource(p.rgb)}
                      className={cn(
                        "u-press size-7 rounded-full border border-border-strong",
                        accentSource === p.rgb && "shadow-[0_0_0_2px_var(--background),0_0_0_4px_var(--text)]",
                      )}
                      style={{ background: p.rgb ? rgb(p.rgb) : "var(--fill)" }}
                    />
                  </Tooltip>
                ))}
              </div>
            </Labeled>
            <Labeled label={`${s.frost} ${Math.round(frost * 100)}%`}>
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={frost}
                aria-label={s.frost}
                onChange={(e) => setFrost(Number(e.target.value))}
                className="w-56 accent-(--accent)"
              />
            </Labeled>
          </div>
        </Section>

        <Section title={s.colors}>
          {TOKENS.map((t) => (
            <div key={t} className="flex w-24 flex-col gap-1.5">
              <div
                className="h-10 rounded-card shadow-[inset_0_0_0_1px_var(--border)]"
                style={{ background: `var(--${t})` }}
              />
              <code className="font-mono text-caption text-text-muted">{t}</code>
            </div>
          ))}
        </Section>

        <Section title={s.typography}>
          <div className="flex w-full flex-col gap-1">
            <p className="text-large-title">{s.display}</p>
            <p className="text-title1">{s.display}</p>
            <p className="text-title2">{s.display}</p>
            <p className="text-title3">{s.display}</p>
            <p className="text-headline">{s.body}</p>
            <p className="text-body text-text">{s.body}</p>
            <p className="text-body text-text-muted">{s.body}</p>
            <p className="text-body text-text-tertiary">{s.body}</p>
            <p className="text-callout text-text-muted">{s.body}</p>
            <p className="text-footnote text-text-muted tabular-nums">3:42 · 1,204 · 10:07</p>
          </div>
        </Section>

        <Section title={s.buttons}>
          <Button variant="primary">
            <Icon as={Play} fill="currentColor" /> {strings.player.play}
          </Button>
          <Button variant="secondary">
            <Icon as={Shuffle} /> {strings.player.shuffle}
          </Button>
          <Button variant="glass">{s.glassButton}</Button>
          <Button variant="ghost">{s.ghost}</Button>
          <Button variant="danger">{strings.accounts.disconnect}</Button>
          <Button variant="primary" disabled>
            {s.disabled}
          </Button>
          <Button variant="secondary" disabled>
            {s.disabled}
          </Button>
          <Button variant="primary" size="sm">
            {s.small}
          </Button>
          <Button variant="primary" size="lg">
            {s.large}
          </Button>
        </Section>

        <Section title={s.iconButtons}>
          <IconButton label={strings.player.previous} size="lg" icon={<Icon as={SkipBack} size={20} />} />
          <IconButton
            label={playing ? strings.player.pause : strings.player.play}
            variant="primary"
            size="xl"
            icon={<Icon as={playing ? Pause : Play} size={20} fill="currentColor" />}
            onClick={() => setPlaying((p) => !p)}
          />
          <IconButton label={strings.player.next} size="lg" icon={<Icon as={SkipForward} size={20} />} />
          <IconButton
            label={strings.player.like}
            active={liked}
            icon={<Icon as={Heart} fill={liked ? "currentColor" : "none"} />}
            onClick={() => setLiked((l) => !l)}
          />
          <IconButton label={strings.player.repeat} active icon={<Icon as={Repeat} />} />
          <IconButton label={strings.player.shuffle} icon={<Icon as={Shuffle} />} />
          <IconButton label={strings.common.more} variant="glass" icon={<Icon as={MoreHorizontal} />} />
          <IconButton label={strings.common.more} disabled icon={<Icon as={MoreHorizontal} />} />
        </Section>

        <Section title={s.inputs}>
          <div className="grid w-full grid-cols-2 gap-4">
            <Labeled label={s.placeholder}>
              <Input
                shape="capsule"
                leading={<Icon as={Search} />}
                placeholder={strings.feedback.searchPlaceholder}
                aria-label={strings.feedback.searchPlaceholder}
              />
            </Labeled>
            <Labeled label={strings.common.filter}>
              <Input
                placeholder={strings.common.filter}
                aria-label={strings.common.filter}
                defaultValue="khmer"
              />
            </Labeled>
            <Labeled label={s.invalid}>
              <Input
                invalid
                defaultValue="not-a-client-id"
                aria-label={strings.accounts.clientId}
                aria-describedby="dev-invalid"
              />
              <span id="dev-invalid" className="text-footnote text-danger">
                {strings.accounts.invalidClientId}
              </span>
            </Labeled>
            <Labeled label={s.disabled}>
              <Input disabled defaultValue={strings.accounts.clientId} aria-label={strings.accounts.clientId} />
            </Labeled>
          </div>
        </Section>

        <Section title={s.selects}>
          <div className="w-44">
            <Select label={strings.common.sort} defaultValue="added" options={sortOptions} />
          </div>
          <div className="w-44">
            <Select label={strings.common.sort} placeholder={strings.common.sort} options={sortOptions} />
          </div>
          <div className="w-44">
            <Select label={strings.common.sort} disabled defaultValue="title" options={sortOptions} />
          </div>
          <div className="w-44">
            <Select label={strings.common.sort} invalid defaultValue="artist" options={sortOptions} />
          </div>
        </Section>

        <Section title={s.segmented}>
          <Segmented label={strings.common.source} value={source} onValueChange={setSource} options={sourceOptions} />
          <Segmented
            label={strings.common.source}
            value="all"
            onValueChange={() => undefined}
            options={sourceOptions}
            disabled
          />
        </Section>

        <Section title={s.menu}>
          <Menu>
            <MenuTrigger asChild>
              <Button variant="secondary">
                <Icon as={MoreHorizontal} /> {s.openMenu}
              </Button>
            </MenuTrigger>
            <MenuContent>
              <MenuItem icon={<Icon as={ListPlus} />}>{strings.track.addToPlaylist}</MenuItem>
              <MenuItem shortcut="Ctrl+Q">{strings.track.addToQueue}</MenuItem>
              <MenuItem disabled>{strings.track.openInSpotify}</MenuItem>
              <MenuSeparator />
              <MenuItem icon={<Icon as={Trash2} />} danger>
                {strings.track.removeFromPlaylist}
              </MenuItem>
            </MenuContent>
          </Menu>
        </Section>

        <Section title={s.dialog}>
          <Dialog>
            <DialogTrigger asChild>
              <Button variant="secondary">{s.openDialog}</Button>
            </DialogTrigger>
            <DialogContent
              icon={<Icon as={Unplug} size={20} />}
              title={strings.accounts.disconnectSpotifyTitle}
              description={strings.accounts.disconnectSpotifyBody}
              footer={
                <>
                  <DialogClose asChild>
                    <Button variant="secondary">{strings.common.cancel}</Button>
                  </DialogClose>
                  <DialogClose asChild>
                    <Button variant="danger">{strings.accounts.disconnect}</Button>
                  </DialogClose>
                </>
              }
            />
          </Dialog>
        </Section>

        <Section title={s.drawer}>
          <Drawer>
            <DrawerTrigger asChild>
              <Button variant="secondary">{s.openDrawer}</Button>
            </DrawerTrigger>
            <DrawerContent
              title={strings.accounts.byokTitle}
              footer={<Button variant="primary">{strings.accounts.connect}</Button>}
            >
              <ol className="flex flex-col gap-5">
                <li className="flex flex-col gap-2">
                  <p className="text-body text-text">1. {strings.accounts.byokStep1}</p>
                  <Button variant="secondary" size="sm" className="self-start">
                    {strings.accounts.openDashboard}
                  </Button>
                </li>
                <li className="flex flex-col gap-2">
                  <p className="text-body text-text">2. {strings.accounts.byokStep2}</p>
                  {REDIRECT_URIS.map((uri) => (
                    <code
                      key={uri}
                      className="rounded-control bg-fill-tertiary px-2.5 py-1.5 font-mono text-callout text-text"
                    >
                      {uri}
                    </code>
                  ))}
                </li>
                <li className="flex flex-col gap-2">
                  <p className="text-body text-text">3. {strings.accounts.byokStep3}</p>
                  <Input placeholder={strings.accounts.clientId} aria-label={strings.accounts.clientId} />
                </li>
              </ol>
            </DrawerContent>
          </Drawer>
        </Section>

        <Section title={s.tooltip}>
          <Tooltip content={s.tooltipText}>
            <Button variant="ghost">{s.hoverMe}</Button>
          </Tooltip>
        </Section>

        <Section title={s.toast}>
          <Button
            variant="secondary"
            onClick={() =>
              toast({
                title: strings.feedback.syncFinished,
                description: format(strings.feedback.syncFinishedBody, { count: 42 }),
              })
            }
          >
            {s.showToast}
          </Button>
          <Button
            variant="secondary"
            onClick={() =>
              toast({
                title: format(strings.feedback.addedToPlaylist, { name: "Late night" }),
                action: { label: strings.common.undo, onAction: () => undefined },
              })
            }
          >
            {s.showUndoToast}
          </Button>
          <Button variant="secondary" onClick={() => toast({ title: strings.feedback.syncFailed, tone: "danger" })}>
            {s.showErrorToast}
          </Button>
        </Section>

        <Section title={s.skeleton}>
          <div className="flex w-full flex-col gap-1">
            {[0, 1, 2].map((i) => (
              <div key={i} className="flex h-14 items-center gap-3">
                <Skeleton className="size-10 rounded-[6px]" />
                <div className="flex flex-1 flex-col gap-2">
                  <Skeleton shape="text" className="w-1/3" />
                  <Skeleton shape="text" className="w-1/5" />
                </div>
                <Skeleton shape="text" className="w-10" />
              </div>
            ))}
          </div>
          <div className="flex gap-4">
            {[0, 1, 2].map((i) => (
              <div key={i} className="flex flex-col gap-2">
                <Skeleton className="size-[180px]" />
                <Skeleton shape="text" className="w-24" />
              </div>
            ))}
            <Skeleton shape="circle" className="size-[180px]" />
          </div>
        </Section>

        <Section title={s.banner}>
          <div className="flex w-full flex-col gap-2">
            <Banner icon={<Icon as={CloudOff} />}>{strings.feedback.offline}</Banner>
            <Banner tone="warning" icon={<Icon as={AlertTriangle} />}>
              {format(strings.feedback.quotaExhausted, { time: "15:00" })}
            </Banner>
            <Banner
              tone="danger"
              action={
                <Button size="sm" variant="secondary">
                  {strings.accounts.reconnect}
                </Button>
              }
              onDismiss={() => undefined}
            >
              {strings.feedback.spotifyExpired}
            </Banner>
          </div>
        </Section>
      </div>
    </div>
  );
}
