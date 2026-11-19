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
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { Link } from "react-router";
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
import { toast } from "@/stores/toast";
import { useUiStore, type Theme } from "@/stores/ui";

const s = strings.devUi;

const TOKENS = [
  "accent",
  "accent-foreground",
  "background",
  "surface",
  "surface-raised",
  "border",
  "border-strong",
  "text",
  "text-muted",
  "text-disabled",
  "success",
  "warning",
  "danger",
] as const;

const REDIRECT_URIS = [43821, 43822, 43823].map((p) => `http://127.0.0.1:${p}/callback`);

type Source = "all" | "spotify" | "youtube";

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="border-t border-border py-8">
      <h2 className="mb-4 font-display text-sm font-medium text-text-muted">{title}</h2>
      <div className="flex flex-wrap items-center gap-4">{children}</div>
    </section>
  );
}

function Labeled({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-xs text-text-muted">{label}</span>
      {children}
    </div>
  );
}

/** Storybook-style gallery: every UI kit component in every state (T-03). */
export function DevUi() {
  const theme = useUiStore((st) => st.theme);
  const setTheme = useUiStore((st) => st.setTheme);
  const [source, setSource] = useState<Source>("all");
  const [playing, setPlaying] = useState(false);
  const [liked, setLiked] = useState(true);

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
      <div className="mx-auto max-w-[960px] px-8 py-8">
        <header className="flex items-center justify-between gap-4 pb-8">
          <h1 className="font-display text-2xl font-medium text-text">{s.title}</h1>
          <div className="flex items-center gap-4">
            <Segmented<Theme>
              label={s.theme}
              value={theme}
              onValueChange={setTheme}
              options={[
                { value: "dark", label: s.dark },
                { value: "light", label: s.light },
              ]}
            />
            <Link to="/" className="text-sm text-text-muted hover:text-text">
              {s.back}
            </Link>
          </div>
        </header>

        <Section title={s.colors}>
          {TOKENS.map((t) => (
            <div key={t} className="flex w-28 flex-col gap-2">
              <div
                className="h-12 rounded-panel border border-border"
                style={{ background: `var(--${t})` }}
              />
              <code className="font-mono text-xs text-text-muted">{t}</code>
            </div>
          ))}
        </Section>

        <Section title={s.typography}>
          <div className="flex w-full flex-col gap-2">
            <p className="font-display text-3xl font-medium tracking-[-0.02em] text-text">
              {s.display}
            </p>
            <p className="text-sm text-text">{s.body}</p>
            <p className="text-sm text-text-muted">{s.body}</p>
            <p className="text-sm text-text-disabled">{s.body}</p>
            <p className="font-mono text-sm text-text-muted tabular-nums">3:42 · 1,204 · 10:07</p>
          </div>
        </Section>

        <Section title={s.buttons}>
          <Button variant="primary">
            <Icon as={Play} /> {strings.player.play}
          </Button>
          <Button variant="secondary">
            <Icon as={Shuffle} /> {strings.player.shuffle}
          </Button>
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
          <Button variant="secondary" size="sm">
            {s.small}
          </Button>
        </Section>

        <Section title={s.iconButtons}>
          <IconButton
            label={strings.player.previous}
            size="lg"
            icon={<Icon as={SkipBack} size={20} />}
          />
          <IconButton
            label={playing ? strings.player.pause : strings.player.play}
            variant="primary"
            size="lg"
            icon={<Icon as={playing ? Pause : Play} size={20} />}
            onClick={() => setPlaying((p) => !p)}
          />
          <IconButton
            label={strings.player.next}
            size="lg"
            icon={<Icon as={SkipForward} size={20} />}
          />
          <IconButton
            label={strings.player.like}
            active={liked}
            icon={<Icon as={Heart} fill={liked ? "currentColor" : "none"} />}
            onClick={() => setLiked((l) => !l)}
          />
          <IconButton label={strings.player.repeat} active icon={<Icon as={Repeat} />} />
          <IconButton label={strings.player.shuffle} icon={<Icon as={Shuffle} />} />
          <IconButton label={strings.common.more} disabled icon={<Icon as={MoreHorizontal} />} />
        </Section>

        <Section title={s.inputs}>
          <div className="grid w-full grid-cols-2 gap-4">
            <Labeled label={s.placeholder}>
              <Input
                placeholder={strings.feedback.searchPlaceholder}
                aria-label={strings.feedback.searchPlaceholder}
              />
            </Labeled>
            <Labeled label={strings.common.filter}>
              <Input
                leading={<Icon as={Search} />}
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
              <span id="dev-invalid" className="text-xs text-danger">
                {strings.accounts.invalidClientId}
              </span>
            </Labeled>
            <Labeled label={s.disabled}>
              <Input
                disabled
                defaultValue={strings.accounts.clientId}
                aria-label={strings.accounts.clientId}
              />
            </Labeled>
          </div>
        </Section>

        <Section title={s.selects}>
          <div className="w-48">
            <Select label={strings.common.sort} defaultValue="added" options={sortOptions} />
          </div>
          <div className="w-48">
            <Select
              label={strings.common.sort}
              placeholder={strings.common.sort}
              options={sortOptions}
            />
          </div>
          <div className="w-48">
            <Select
              label={strings.common.sort}
              disabled
              defaultValue="title"
              options={sortOptions}
            />
          </div>
          <div className="w-48">
            <Select
              label={strings.common.sort}
              invalid
              defaultValue="artist"
              options={sortOptions}
            />
          </div>
        </Section>

        <Section title={s.segmented}>
          <Segmented
            label={strings.common.source}
            value={source}
            onValueChange={setSource}
            options={sourceOptions}
          />
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
              title={strings.accounts.disconnectSpotifyTitle}
              description={strings.accounts.disconnectSpotifyBody}
              footer={
                <>
                  <DialogClose asChild>
                    <Button variant="ghost">{strings.common.cancel}</Button>
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
              <ol className="flex flex-col gap-6">
                <li className="flex flex-col gap-2">
                  <p className="text-sm text-text">1. {strings.accounts.byokStep1}</p>
                  <Button variant="secondary" className="self-start">
                    {strings.accounts.openDashboard}
                  </Button>
                </li>
                <li className="flex flex-col gap-2">
                  <p className="text-sm text-text">2. {strings.accounts.byokStep2}</p>
                  {REDIRECT_URIS.map((uri) => (
                    <code
                      key={uri}
                      className="rounded-control bg-surface-raised px-2 py-1 font-mono text-xs text-text"
                    >
                      {uri}
                    </code>
                  ))}
                </li>
                <li className="flex flex-col gap-2">
                  <p className="text-sm text-text">3. {strings.accounts.byokStep3}</p>
                  <Input
                    placeholder={strings.accounts.clientId}
                    aria-label={strings.accounts.clientId}
                  />
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
          <Button
            variant="secondary"
            onClick={() => toast({ title: strings.feedback.syncFailed, tone: "danger" })}
          >
            {s.showErrorToast}
          </Button>
        </Section>

        <Section title={s.skeleton}>
          <div className="flex w-full flex-col gap-2">
            {[0, 1, 2].map((i) => (
              <div key={i} className="flex h-14 items-center gap-3">
                <Skeleton className="size-10" />
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
