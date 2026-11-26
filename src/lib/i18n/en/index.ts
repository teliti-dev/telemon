import about from "./about"
import admin from "./admin"
import ai from "./ai"
import aidash from "./aidash"
import auth from "./auth"
import common from "./common"
import dashboard from "./dashboard"
import data from "./data"
import events from "./events"
import lists from "./lists"
import profiles from "./profiles"
import routers from "./routers"
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
  ...shell,
  ...auth,
  ...users,
  ...events,
  ...lists,
  ...profiles,
  ...routers,
  ...admin,
  ...ai,
  ...update,
  ...about,
  ...aidash,
  ...settings,
  ...data,
}
