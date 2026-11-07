package app.pley.android.ui.phone.admin.requests

import androidx.compose.runtime.Composable
import app.pley.android.screens.PleyKey
import app.pley.android.screens.admin.AdminSection
import app.pley.android.ui.phone.shell.Placeholder

/** This part's page; [id]: one thing of it (see [PleyKey.Manage]). Built in the part's own change. */
@Composable
internal fun RequestsAdminScreen(id: String?) {
    Placeholder(PleyKey.Manage(AdminSection.Requests, id))
}
