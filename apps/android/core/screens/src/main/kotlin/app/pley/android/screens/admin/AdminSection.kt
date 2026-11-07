package app.pley.android.screens.admin

import kotlinx.serialization.Serializable

/**
 * The parts of managing the server, in the order of Settings in the web app
 * (apps/web/src/pages/settings/SettingsLayout.tsx), Dashboard first.
 */
@Serializable
enum class AdminSection {
    Dashboard,
    Libraries,
    LiveTv,
    Users,
    Invitations,
    Requests,
    Server,
    Transcoder,
    Network,
}
