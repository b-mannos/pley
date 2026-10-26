' The tokens of each server, in the registry (roRegistrySection "tokens",
' one key per server). A refresh token works once, so a new pair is written
' and flushed before the new access token is used. Only the API task
' writes here. section is a roRegistrySection, or a stand-in with the same
' methods in tests.

function tokensKey(issuer as string) as string
    return "server:" + issuer
end function

' The tokens kept for issuer: { issuer, tokenEndpoint, revocationEndpoint,
' access, refresh, expiresAt, scope }, or invalid.
function tokensLoad(section as object, issuer as string) as dynamic
    key = tokensKey(issuer)
    if not section.Exists(key) then return invalid
    data = httpObject(httpJson(section.Read(key)))
    if data = invalid then return invalid
    tokens = {
        issuer: issuer,
        tokenEndpoint: httpString(data.token_endpoint),
        revocationEndpoint: httpString(data.revocation_endpoint),
        access: httpString(data.access),
        refresh: httpString(data.refresh),
        expiresAt: httpNumber(data.expires_at, 0),
        scope: httpString(data.scope)
    }
    if tokens.access = "" or tokens.tokenEndpoint = "" or tokens.revocationEndpoint = "" then return invalid
    return tokens
end function

' Writes tokens and flushes the registry; false when either failed.
function tokensSave(section as object, tokens as object) as boolean
    text = FormatJson({
        token_endpoint: tokens.tokenEndpoint,
        revocation_endpoint: tokens.revocationEndpoint,
        access: tokens.access,
        refresh: tokens.refresh,
        expires_at: tokens.expiresAt,
        scope: tokens.scope
    })
    if not section.Write(tokensKey(tokens.issuer), text) then return false
    return section.Flush()
end function

function tokensWipe(section as object, issuer as string) as boolean
    section.Delete(tokensKey(issuer))
    return section.Flush()
end function

' Tokens as a sign-in or refresh gave them (authParseTokens), kept for the
' endpoints they came from.
function tokensFrom(answer as object, issuer as string, tokenEndpoint as string, revocationEndpoint as string) as object
    return {
        issuer: issuer,
        tokenEndpoint: tokenEndpoint,
        revocationEndpoint: revocationEndpoint,
        access: answer.access,
        refresh: answer.refresh,
        expiresAt: answer.expiresAt,
        scope: answer.scope
    }
end function
