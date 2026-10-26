sub test_transfer_identities_stay_apart()
    inUse = {}
    for i = 1 to 60
        transfer = transferNew(inUse)
        key = transfer.GetIdentity().ToStr()
        tkFalse(inUse.DoesExist(key), "identity " + key + " given twice")
        inUse[key] = transfer
    end for
    tkEqual(inUse.Count(), 60)
end sub
