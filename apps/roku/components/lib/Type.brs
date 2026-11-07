' Pley's type on Roku: Roboto (fonts/, OFL-1.1), the face of the Android
' app and the other TVs, in weights 400, 500 and 700 only, at Android
' TV's sizes (PleyType.Tv) doubled from its 960 × 540 canvas to Roku's
' 1920 × 1080. The Font nodes are made in shared/Type.brs.

' One of Android TV's styles: { weight, size } in pixels at 1080p.
function typeStyle(name as string) as object
    styles = {
        ' A title without logo art, on Home's hero.
        heroTitle: { weight: 700, size: 76 },
        ' A page's title (Library, Search, Settings, a title's page).
        pageTitle: { weight: 700, size: 56 },
        ' Row headers.
        section: { weight: 700, size: 36 },
        ' Panel and dialog titles, an info block's title, a list's names.
        title: { weight: 700, size: 32 },
        ' Summaries on the hero and a title's page.
        bodyLarge: { weight: 400, size: 30 },
        ' Lists, settings, instructions.
        body: { weight: 400, size: 28 },
        ' Buttons, the metadata line, tabs, chips, the rail.
        label: { weight: 500, size: 28 },
        ' Tile captions, times.
        caption: { weight: 500, size: 26 },
        ' Quality badges and numbers in boxes.
        badge: { weight: 700, size: 22 }
    }
    style = styles[name]
    if style = invalid then return styles.body
    return style
end function

' text in capitals spaced out, as Android TV's hero writes why a title is
' there ("NEW IN MOVIES", 3 sp apart): a thin space after each character,
' 5.7 pixels in Roboto at 28. Roku's own font, which draws what Roboto
' lacks, may lack the thin space: such text is in capitals alone.
function typeSpaced(text as string) as string
    upper = UCase(text)
    if not typeCovers(upper) then return upper
    thin = Chr(&h2009)
    out = ""
    for i = 1 to Len(upper)
        if i > 1 then out = out + thin
        out = out + Mid(upper, i, 1)
    end for
    return out
end function

' Whether Pley's Roboto, its Latin files, draws every character of text:
' Roku draws a label in one font, without falling back to another for a
' character it lacks, so a title in Greek, Cyrillic, Polish or Japanese
' takes Roku's own font (typeSet). The characters are those the files'
' character maps hold (scripts/fonts.mjs).
function typeCovers(text as string) as boolean
    bytes = CreateObject("roByteArray")
    bytes.FromAsciiString(text)
    count = bytes.Count()
    ' Every character one byte: ASCII.
    if count = Len(text) then return true
    i = 0
    while i < count
        lead = bytes[i]
        if lead < &h80
            i = i + 1
        else if lead >= &hC0 and lead < &hE0 and i + 1 < count
            code = ((lead and &h1F) << 6) or (bytes[i + 1] and &h3F)
            if (code < &hA0 or code > &hFF) and not typeExtra(code) then return false
            i = i + 2
        else if lead >= &hE0 and lead < &hF0 and i + 2 < count
            code = ((lead and &h0F) << 12) or ((bytes[i + 1] and &h3F) << 6) or (bytes[i + 2] and &h3F)
            if not typeExtra(code) then return false
            i = i + 3
        else
            ' Beyond the Basic Multilingual Plane (emoji), or broken.
            return false
        end if
    end while
    return true
end function

' Whether Roboto's Latin files have the character code past Latin-1: the
' dotless i, the œ ligature, a few accents and the punctuation of
' General Punctuation they hold (dashes, quotes, the ellipsis, the bullet),
' the euro, the trade mark and the minus.
function typeExtra(code as integer) as boolean
    if code >= &h2018 and code <= &h201A then return true
    if code >= &h201C and code <= &h201E then return true
    known = [&h131, &h152, &h153, &h2BC, &h2C6, &h2DA, &h2DC, &h300, &h301, &h303, &h309, &h323, &h2002, &h2009, &h200B, &h2013, &h2014, &h2022, &h2026, &h2032, &h2033, &h2039, &h203A, &h2044, &h20AC, &h2122, &h2212, &hFEFF, &hFFFD]
    for each one in known
        if one = code then return true
    end for
    return false
end function

' Roku's own font nearest Roboto at size (weight 700 bold): what a label
' falls back to.
function typeSystemFont(size as integer, weight as integer) as string
    name = "ExtraLarge"
    if size <= 25
        name = "Tiny"
    else if size <= 28
        name = "Smallest"
    else if size <= 31
        name = "Smaller"
    else if size <= 34
        name = "Small"
    else if size <= 40
        name = "Medium"
    else if size <= 49
        name = "Large"
    end if
    if weight >= 700 then name = name + "Bold"
    return "font:" + name + "SystemFont"
end function
