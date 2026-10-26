' The API task's transfers (never the render thread's).

' A new roUrlTransfer whose identity none of inUse's keys has (the
' identities of the transfers under way, as strings): its answer is told
' from theirs by it. A Roku's identities are unique; brs-node, which runs
' the tests and the smoke run, draws them at random from 0 to 79.
function transferNew(inUse as object) as object
    transfer = CreateObject("roUrlTransfer")
    while inUse.DoesExist(transfer.GetIdentity().ToStr())
        transfer = CreateObject("roUrlTransfer")
    end while
    return transfer
end function
