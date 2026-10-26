sub test_http_user_agent_names_app_os_and_model()
    os = { major: "15", minor: "0", revision: "4", build: "4200" }
    tkEqual(httpUserAgent("0.1.0", os, "Roku Ultra"), "pleyRoku/0.1.0 (Roku OS 15.0.4; Roku Ultra)")
    tkEqual(httpUserAgent("0.1.0", invalid, "Roku Express"), "pleyRoku/0.1.0 (Roku OS ?; Roku Express)")
    tkEqual(httpUserAgent("0.1.0", os, "TV (55"";) A" + Chr(10)), "pleyRoku/0.1.0 (Roku OS 15.0.4; TV 55"" A)")
    tkEqual(httpUserAgent("", os, ""), "pleyRoku/0 (Roku OS 15.0.4; Roku)")
end sub

sub test_http_sentences_are_one_safe_line()
    tkEqual(httpSentence("  the code was used already" + Chr(10) + "  "), "the code was used already")
    tkEqual(httpSentence("a" + Chr(&h202E) + "b" + Chr(7) + "c"), "ab c", "direction override gone, control a space")
    tkEqual(httpSentence("one" + Chr(13) + Chr(10) + "two"), "one two")
    tkEqual(httpSentence("Amélie" + Chr(&h200B)), "Amélie")
    tkEqual(Len(httpSentence(String(300, "x"))), 200)
    tkEqual(httpSentence(invalid), "")
    tkEqual(httpSentence(42), "")
end sub

sub test_http_content_text_keeps_its_lines_and_length()
    tkEqual(httpClean("Line one." + Chr(10) + "Line two." + Chr(&h200F) + Chr(7), 300, true), "Line one." + Chr(10) + "Line two.")
    tkEqual(Len(httpClean(String(500, "x"), 300, true)), 300)
    tkEqual(Len(httpClean(String(900, "y"), 1200, true)), 900, "a summary isn't cut at a message's length")
end sub

sub test_http_reads_bearer_invalid_token_challenges()
    tkTrue(httpInvalidToken("Bearer error=""invalid_token"""))
    tkTrue(httpInvalidToken("Bearer realm=""pley"", error=""invalid_token"", error_description=""expired"""))
    tkTrue(httpInvalidToken("bearer error=invalid_token"))
    tkFalse(httpInvalidToken("Bearer error=""insufficient_scope"", scope=""play"""))
    tkFalse(httpInvalidToken("Bearer error=""invalid_request"""))
    tkFalse(httpInvalidToken("Basic realm=""pley"""))
    tkFalse(httpInvalidToken("BearerX error=""invalid_token"""))
    tkFalse(httpInvalidToken(invalid))
end sub

sub test_http_answers_carry_data_and_errors()
    ok = httpAnswer(200, "{""status"":""ok""}")
    tkTrue(ok.ok)
    tkEqual(ok.data, { status: "ok" })
    tkEqual(httpAnswer(204, "").data, invalid)

    missing = httpAnswer(404, "{""code"":""not_found"",""message"":""item 42 not found""}")
    tkFalse(missing.ok)
    tkEqual(missing.error, "not_found")
    tkEqual(missing.message, "item 42 not found")

    oauth = httpAnswer(400, "{""error"":""invalid_grant"",""error_description"":""refresh token used again: the app's access is ended""}")
    tkEqual(oauth.error, "invalid_grant")
    tkEqual(oauth.message, "refresh token used again: the app's access is ended")

    offline = httpAnswer(-7, "")
    tkEqual(offline.error, "network")
    tkEqual(offline.message, "The server can't be reached.")

    tkEqual(httpAnswer(500, "").message, "The server couldn't answer (HTTP 500).")
end sub

sub test_http_a_proxy_answering_for_the_server_is_no_answer()
    tkEqual(httpAnswer(502, "<html>bad gateway</html>").error, "network")
    tkEqual(httpAnswer(504, "").error, "network")
    tkEqual(httpAnswer(503, "<html>maintenance</html>").error, "network")
    busy = httpAnswer(503, "{""code"":""unavailable"",""message"":""the server is already running its maximum of 2 transcodes""}")
    tkEqual(busy.error, "unavailable", "pley's own 503")
    tkEqual(busy.message, "the server is already running its maximum of 2 transcodes")
end sub

sub test_http_reads_json_values_by_type()
    tkEqual(httpString("x"), "x")
    tkEqual(httpString(1), "")
    tkEqual(httpNumber(5, 0), 5)
    tkEqual(httpNumber(ParseJson("5000000000"), 0), ParseJson("5000000000"))
    tkEqual(httpNumber("5", -1), -1)
    tkTrue(httpTrue(true))
    tkFalse(httpTrue(invalid))
    tkFalse(httpTrue("true"))
    tkEqual(httpArray(invalid), [])
    tkInvalid(httpObject([]))
end sub
