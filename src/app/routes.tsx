import type { RouteObject } from "react-router";
import { Shell } from "@/components/shell/Shell";
import { Album } from "@/routes/album/Album";
import { Artist } from "@/routes/artist/Artist";
import { DevUi } from "@/routes/dev/DevUi";
import { Home } from "@/routes/home/Home";
import { Albums } from "@/routes/library/Albums";
import { Artists } from "@/routes/library/Artists";
import { Liked } from "@/routes/library/Liked";
import { Playlists } from "@/routes/library/Playlists";
import { Playlist } from "@/routes/playlist/Playlist";
import { Search } from "@/routes/search/Search";
import { Settings } from "@/routes/settings/Settings";

export const routes: RouteObject[] = [
  {
    element: <Shell />,
    children: [
      { path: "/", element: <Home /> },
      { path: "/search", element: <Search /> },
      { path: "/library/liked", element: <Liked /> },
      { path: "/library/albums", element: <Albums /> },
      { path: "/library/playlists", element: <Playlists /> },
      { path: "/library/artists", element: <Artists /> },
      { path: "/playlist/:id", element: <Playlist /> },
      { path: "/album/:id", element: <Album /> },
      { path: "/artist/:id", element: <Artist /> },
      { path: "/settings", element: <Settings /> },
      ...(import.meta.env.DEV ? [{ path: "/dev/ui", element: <DevUi /> }] : []),
      { path: "*", element: <Home /> },
    ],
  },
];
