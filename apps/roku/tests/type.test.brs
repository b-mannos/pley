sub test_type_covers_latin()
    tkTrue(typeCovers(""))
    tkTrue(typeCovers("The Grand Budapest Hotel"))
    tkTrue(typeCovers("Amélie · Le Fabuleux Destin d'Amélie Poulain"), "Latin-1")
    tkTrue(typeCovers("Æon Flux – “Thé” … Œuvre €5"), "dashes, quotes, the ellipsis, œ, the euro")
    tkTrue(typeCovers("Straße · 2019 · 1 h 42 min"), "ß and the middle dot")
end sub

sub test_type_covers_not_other_scripts()
    tkFalse(typeCovers("Łódź"), "Latin Extended-A")
    tkFalse(typeCovers("Θ Ελληνικά"), "Greek")
    tkFalse(typeCovers("Москва"), "Cyrillic")
    tkFalse(typeCovers("千と千尋の神隠し"), "Japanese")
    tkFalse(typeCovers("★ 7.5"), "the star, drawn as an icon")
    tkFalse(typeCovers("Settings → Server"), "the arrow")
    tkFalse(typeCovers("Party 🎉"), "emoji")
end sub

sub test_type_system_fonts()
    tkEqual(typeSystemFont(24, 500), "font:TinySystemFont")
    tkEqual(typeSystemFont(26, 700), "font:SmallestBoldSystemFont")
    tkEqual(typeSystemFont(30, 400), "font:SmallerSystemFont")
    tkEqual(typeSystemFont(32, 700), "font:SmallBoldSystemFont")
    tkEqual(typeSystemFont(36, 700), "font:MediumBoldSystemFont")
    tkEqual(typeSystemFont(44, 700), "font:LargeBoldSystemFont")
    tkEqual(typeSystemFont(76, 700), "font:ExtraLargeBoldSystemFont")
end sub

sub test_type_styles()
    tkEqual(typeStyle("heroTitle").size, 76)
    tkEqual(typeStyle("label").weight, 500)
    tkEqual(typeStyle("caption").size, 26)
    tkEqual(typeStyle("unknown").size, typeStyle("body").size, "an unknown style is body text")
end sub
