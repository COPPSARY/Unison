import type { RouteObject } from "react-router";
import { Shell } from "@/components/shell/Shell";
import { Album } from "@/routes/album/Album";
import { Artist } from "@/routes/artist/Artist";
import { DevUi } from "@/routes/dev/DevUi";
import { ExtensionDetail } from "@/routes/extensions/ExtensionDetail";
import { Extensions } from "@/routes/extensions/Extensions";
import { Home } from "@/routes/home/Home";
import { Albums } from "@/routes/library/Albums";
import { Artists } from "@/routes/library/Artists";
import { Liked } from "@/routes/library/Liked";
import { Local } from "@/routes/library/Local";
import { Playlists } from "@/routes/library/Playlists";
import { Playlist } from "@/routes/playlist/Playlist";
import { Search } from "@/routes/search/Search";
import { Settings } from "@/routes/settings/Settings";
import { Track } from "@/routes/track/Track";

export const routes: RouteObject[] = [
  {
    element: <Shell />,
    children: [
      { path: "/", element: <Home /> },
      { path: "/search", element: <Search /> },
      { path: "/extensions", element: <Extensions /> },
      { path: "/extensions/:key", element: <ExtensionDetail /> },
      { path: "/track/:key", element: <Track /> },
      { path: "/library/liked", element: <Liked /> },
      { path: "/library/albums", element: <Albums /> },
      { path: "/library/playlists", element: <Playlists /> },
      { path: "/library/artists", element: <Artists /> },
      { path: "/library/local", element: <Local /> },
      { path: "/playlist/:id", element: <Playlist /> },
      { path: "/album/:id", element: <Album /> },
      { path: "/artist/:id", element: <Artist /> },
      { path: "/settings", element: <Settings /> },
      ...(import.meta.env.DEV ? [{ path: "/dev/ui", element: <DevUi /> }] : []),
      { path: "*", element: <Home /> },
    ],
  },
];
