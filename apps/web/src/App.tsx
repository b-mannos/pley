import type { ComponentType } from "react";
import { createBrowserRouter, Navigate, RouterProvider, type RouteObject } from "react-router";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { LoaderCircle } from "lucide-react";
import { ApiError, setUnauthorizedHandler } from "@/api/client";
import { queries } from "@/api/hooks";
import { RequireAuth } from "@/auth";
import { AppLayout } from "@/components/AppLayout";
import { ToastProvider } from "@/components/Toast";
import { PageSpinner } from "@/components/ui";
import { AccountPage } from "@/pages/AccountPage";
import { HomePage } from "@/pages/HomePage";
import { InvitePage } from "@/pages/InvitePage";
import { ItemPage } from "@/pages/item/ItemPage";
import { LibraryPage } from "@/pages/LibraryPage";
import { LoginPage } from "@/pages/LoginPage";
import { NotFoundPage, RouteErrorPage } from "@/pages/NotFoundPage";
import { PersonPage } from "@/pages/PersonPage";
import { ProfilesPage } from "@/pages/ProfilesPage";
import { SearchPage } from "@/pages/SearchPage";
import { SetupPage } from "@/pages/SetupPage";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 10 * 60_000,
      refetchOnWindowFocus: false,
      retry: (count, err) => {
        if (err instanceof ApiError && [400, 401, 403, 404, 409].includes(err.status)) return false;
        return count < 2;
      },
    },
    mutations: { retry: false },
  },
});

function PlayerFallback() {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black">
      <LoaderCircle className="size-12 animate-spin text-white/70" />
    </div>
  );
}

/**
 * A route whose code (the player with hls.js, the admin screens) loads on
 * demand, together with the data the page opens with: the router waits for
 * both before showing it, so it appears complete, without a chain of
 * requests (code, then data) or a spinner in between.
 */
function onDemand<M>(
  load: () => Promise<M>,
  component: (module: M) => ComponentType,
  data: (prefetch: QueryClient["prefetchQuery"]) => Promise<void>[] = () => [],
  fallback = <PageSpinner />,
): Pick<RouteObject, "lazy" | "loader" | "hydrateFallbackElement"> {
  return {
    lazy: async () => ({ Component: component(await load()) }),
    // Failures are the page's to show (or RequireAdmin's): prefetching never throws.
    loader: async () => {
      await Promise.all(data(queryClient.prefetchQuery.bind(queryClient)));
      return null;
    },
    hydrateFallbackElement: fallback,
  };
}

const router = createBrowserRouter([
  { path: "/setup", element: <SetupPage />, errorElement: <RouteErrorPage /> },
  { path: "/login", element: <LoginPage />, errorElement: <RouteErrorPage /> },
  { path: "/invite/:code", element: <InvitePage />, errorElement: <RouteErrorPage /> },
  {
    element: <RequireAuth />,
    errorElement: <RouteErrorPage />,
    children: [
      { path: "/profiles", element: <ProfilesPage /> },
      {
        path: "/play/:id",
        ...onDemand(
          () => import("@/player/PlayerPage"),
          (m) => m.PlayerPage,
          () => [],
          <PlayerFallback />,
        ),
      },
      {
        element: <AppLayout />,
        children: [
          { index: true, element: <HomePage /> },
          { path: "library/:id", element: <LibraryPage /> },
          { path: "item/:id", element: <ItemPage /> },
          { path: "person/:id", element: <PersonPage /> },
          { path: "collection/:id", ...onDemand(() => import("@/pages/collections/CollectionPage"), (m) => m.CollectionPage) },
          { path: "playlists", ...onDemand(() => import("@/pages/playlists/PlaylistsPage"), (m) => m.PlaylistsPage) },
          { path: "playlists/:id", ...onDemand(() => import("@/pages/playlists/PlaylistPage"), (m) => m.PlaylistPage) },
          { path: "search", element: <SearchPage /> },
          { path: "account", element: <AccountPage /> },
          {
            path: "settings",
            ...onDemand(() => import("@/pages/settings/SettingsLayout"), (m) => m.SettingsLayout),
            children: [
              { index: true, element: <Navigate to="dashboard" replace /> },
              {
                path: "libraries",
                ...onDemand(
                  () => import("@/pages/settings/LibrariesSettings"),
                  (m) => m.LibrariesSettings,
                  (prefetch) => [prefetch(queries.libraries()), prefetch(queries.activity())],
                ),
              },
              {
                path: "users",
                ...onDemand(
                  () => import("@/pages/settings/UsersSettings"),
                  (m) => m.UsersSettings,
                  (prefetch) => [prefetch(queries.users()), prefetch(queries.libraries())],
                ),
              },
              {
                path: "invitations",
                ...onDemand(
                  () => import("@/pages/settings/InvitesSettings"),
                  (m) => m.InvitesSettings,
                  (prefetch) => [prefetch(queries.invites()), prefetch(queries.libraries())],
                ),
              },
              {
                path: "server",
                ...onDemand(
                  () => import("@/pages/settings/ServerSettings"),
                  (m) => m.ServerSettings,
                  (prefetch) => [prefetch(queries.settings()), prefetch(queries.system())],
                ),
              },
              {
                path: "dashboard",
                ...onDemand(
                  () => import("@/pages/settings/DashboardSettings"),
                  (m) => m.DashboardSettings,
                  (prefetch) => [prefetch(queries.activity()), prefetch(queries.system())],
                ),
              },
            ],
          },
          { path: "*", element: <NotFoundPage /> },
        ],
      },
    ],
  },
]);

// Any authenticated request that comes back 401 (expired/revoked session) sends the user to the login screen.
setUnauthorizedHandler(() => {
  const { pathname, search } = window.location;
  if (pathname.startsWith("/login") || pathname.startsWith("/setup") || pathname.startsWith("/invite/")) return;
  const next = pathname + search;
  void router
    .navigate(next && next !== "/" ? `/login?next=${encodeURIComponent(next)}` : "/login", { replace: true })
    .then(() => queryClient.clear());
});

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <RouterProvider router={router} />
      </ToastProvider>
    </QueryClientProvider>
  );
}
