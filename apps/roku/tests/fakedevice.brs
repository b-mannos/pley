' A stand-in for roDeviceInfo that answers from spec:
'   video: { "<codec>": { "<profile>": highest level, containers: [...] } }
'   audio: codecs decoded; passthrough: codecs sent as they are
'   display: GetDisplayProperties(); output: GetAudioOutputChannel();
'   mode: GetVideoMode(); model: GetModelDisplayName()
' and keeps the questions it was asked.
function fakeDevice(spec as object) as object
    return {
        spec: spec,
        asked: [],
        CanDecodeVideo: function(format as object) as object
            m.asked.Push(format)
            codec = m.spec.video[format.codec]
            if codec = invalid then return { result: false, updated: "codec" }
            if format.container <> invalid and codec.containers <> invalid
                listed = false
                for each container in codec.containers
                    if container = format.container then listed = true
                end for
                if not listed then return { result: false, updated: "container" }
            end if
            if format.profile <> invalid
                highest = codec[format.profile]
                if highest = invalid then return { result: false, updated: "profile" }
                if format.level <> invalid and Val(format.level) > highest then return { result: false, updated: "level" }
            end if
            return { result: true }
        end function,
        CanDecodeAudio: function(format as object) as object
            m.asked.Push(format)
            list = m.spec.audio
            if format.passthru <> invalid then list = m.spec.passthrough
            for each codec in list
                if codec = format.codec then return { result: true }
            end for
            return { result: false, updated: "codec" }
        end function,
        GetDisplayProperties: function() as object
            return m.spec.display
        end function,
        GetAudioOutputChannel: function() as string
            return m.spec.output
        end function,
        GetVideoMode: function() as string
            return m.spec.mode
        end function,
        GetModelDisplayName: function() as string
            return m.spec.model
        end function,
        GetModel: function() as string
            return "4850X"
        end function
    }
end function

' A Roku Ultra on a 4K HDR TV with a surround receiver.
function fakeUltra() as object
    return fakeDevice({
        video: {
            "mpeg4 avc": { "main": 5.1, "high": 5.1 },
            "hevc": { "main": 5.1, "main 10": 5.1 },
            "vp9": { "profile 0": 5.1, "profile 2": 5.1, containers: ["mkv", "webm"] },
            "av1": { "main": 5.1, containers: [] }
        },
        audio: ["aac", "mp3", "ac3", "eac3", "flac", "alac", "opus", "vorbis", "pcm"],
        passthrough: ["ac3", "eac3", "dts"],
        display: { Width: 160, Height: 90, Internal: false, Hdr10: true, Hdr10Plus: true, HdrSeamless: false, HLG: true, DolbyVision: true },
        output: "5.1 surround",
        mode: "2160p60b10",
        model: "Roku Ultra"
    })
end function

' A Roku Express on a 1080p TV's speakers.
function fakeExpress() as object
    return fakeDevice({
        video: { "mpeg4 avc": { "main": 4.2, "high": 4.2 } },
        audio: ["aac", "mp3"],
        passthrough: ["ac3", "eac3"],
        display: { Hdr10: false, Hdr10Plus: false, HLG: false, DolbyVision: false },
        output: "Stereo",
        mode: "1080p",
        model: "Roku Express"
    })
end function
