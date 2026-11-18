import * as React from "react"

import { useApi } from "./api-context"
import type { LiveUpdate, Snapshot } from "./data/api"
import { StoreContext, type StoreState } from "./store-context"

const MAX_EVENTS = 1000

type Action =
  | { type: "loading" }
  | { type: "snapshot"; snap: Snapshot }
  | { type: "error"; error: string }
  | { type: "live"; update: LiveUpdate }

function reducer(state: StoreState, action: Action): StoreState {
  switch (action.type) {
    case "loading":
      return { ...state, status: "loading", error: undefined }
    case "error":
      return { ...state, status: "error", error: action.error }
    case "snapshot":
      return {
        status: "ready",
        routers: action.snap.routers,
        users: action.snap.users,
        profiles: action.snap.profiles,
        events: action.snap.events.slice(0, MAX_EVENTS),
        metas: action.snap.metas,
        share: action.snap.share,
      }
    case "live": {
      const u = action.update
      switch (u.kind) {
        case "event": {
          const events = [u.event, ...state.events].slice(0, MAX_EVENTS)
          const users = u.user
            ? state.users.map((x) => (x.id === u.user!.id ? u.user! : x))
            : state.users
          const routers = u.router
            ? state.routers.map((x) => (x.id === u.router!.id ? u.router! : x))
            : state.routers
          return { ...state, events, users, routers }
        }
        case "router": {
          const exists = state.routers.some((r) => r.id === u.router.id)
          return {
            ...state,
            routers: exists
              ? state.routers.map((r) => (r.id === u.router.id ? u.router : r))
              : [...state.routers, u.router],
          }
        }
        case "router-removed":
          return {
            ...state,
            routers: state.routers.filter((r) => r.id !== u.routerId),
            users: state.users.filter((x) => x.routerId !== u.routerId),
            profiles: state.profiles.filter((x) => x.routerId !== u.routerId),
          }
        case "user": {
          const exists = state.users.some((x) => x.id === u.user.id)
          return {
            ...state,
            users: exists ? state.users.map((x) => (x.id === u.user.id ? u.user : x)) : [...state.users, u.user],
          }
        }
        case "profile": {
          const exists = state.profiles.some((x) => x.id === u.profile.id)
          return {
            ...state,
            profiles: exists
              ? state.profiles.map((x) => (x.id === u.profile.id ? u.profile : x))
              : [...state.profiles, u.profile],
          }
        }
        case "inventory":
          return {
            ...state,
            users: [...state.users.filter((x) => x.routerId !== u.routerId), ...u.users],
            profiles: [...state.profiles.filter((x) => x.routerId !== u.routerId), ...u.profiles],
          }
        case "user-meta":
          return { ...state, metas: { ...state.metas, [u.userId]: u.meta } }
        case "share":
          return { ...state, share: u.share }
      }
    }
  }
}

const initial: StoreState = {
  status: "loading",
  routers: [],
  users: [],
  profiles: [],
  events: [],
  metas: {},
}

export function StoreProvider({ children }: { children: React.ReactNode }) {
  const { api, access } = useApi()
  const [state, dispatch] = React.useReducer(reducer, initial)
  const [attempt, setAttempt] = React.useState(0)

  React.useEffect(() => {
    let cancelled = false
    api
      .snapshot()
      .then((snap) => !cancelled && dispatch({ type: "snapshot", snap }))
      .catch(
        (e: unknown) =>
          !cancelled && dispatch({ type: "error", error: String(e) })
      )
    const unsubscribe = api.subscribe((update) =>
      dispatch({ type: "live", update })
    )
    return () => {
      cancelled = true
      unsubscribe()
    }
  }, [api, attempt])

  const reload = React.useCallback(() => {
    dispatch({ type: "loading" })
    setAttempt((n) => n + 1)
  }, [])

  const routerById = React.useMemo(
    () => new Map(state.routers.map((r) => [r.id, r])),
    [state.routers]
  )

  const liveUsers = React.useMemo(() => state.users.filter((u) => !u.deletedAt), [state.users])

  const value = React.useMemo(
    () => ({ ...state, api, access, routerById, liveUsers, reload }),
    [state, api, access, routerById, liveUsers, reload]
  )

  return <StoreContext.Provider value={value}>{children}</StoreContext.Provider>
}
