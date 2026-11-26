import about from "./about"
import admin from "./admin"
import ai from "./ai"
import aiplus from "./aiplus"
import aidash from "./aidash"
import auth from "./auth"
import bandwidth from "./bandwidth"
import common from "./common"
import dashboard from "./dashboard"
import data from "./data"
import events from "./events"
import extensions from "./extensions"
import lists from "./lists"
import profiles from "./profiles"
import routers from "./routers"
import search from "./search"
import settings from "./settings"
import shell from "./shell"
import update from "./update"
import users from "./users"

/**
 * Kamus English. Kunci = teks Indonesia persis seperti di kode (sumber teks).
 * Tiap area punya berkas sendiri agar mudah dirawat dan tidak bentrok.
 */
export const EN: Record<string, string> = {
  ...common,
  ...dashboard,
  ...bandwidth,
  ...aiplus,
  ...shell,
  ...auth,
  ...users,
  ...events,
  ...extensions,
  ...lists,
  ...profiles,
  ...routers,
  ...search,
  ...admin,
  ...ai,
  ...update,
  ...about,
  ...aidash,
  ...settings,
  ...data,
}
