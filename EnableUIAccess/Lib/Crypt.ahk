class Crypt
{
    ;   Provider Types
static PROV_RSA_FULL      := 1
    ,  PROV_RSA_SIG       := 2
    ,  PROV_DSS           := 3
    ,  PROV_FORTEZZA      := 4
    ,  PROV_MS_EXCHANGE   := 5
    ,  PROV_SSL           := 6
    ,  PROV_STT_MER       := 7  ; <= XP
    ,  PROV_STT_ACQ       := 8  ; <= XP
    ,  PROV_STT_BRND      := 9  ; <= XP
    ,  PROV_STT_ROOT      := 10 ; <= XP
    ,  PROV_STT_ISS       := 11 ; <= XP
    ,  PROV_RSA_SCHANNEL  := 12
    ,  PROV_DSS_DH        := 13
    ,  PROV_EC_ECDSA_SIG  := 14
    ,  PROV_EC_ECNRA_SIG  := 15
    ,  PROV_EC_ECDSA_FULL := 16
    ,  PROV_EC_ECNRA_FULL := 17
    ,  PROV_DH_SCHANNEL   := 18
    ,  PROV_SPYRUS_LYNKS  := 20
    ,  PROV_RNG           := 21
    ,  PROV_INTEL_SEC     := 22
    ,  PROV_REPLACE_OWF   := 23 ; >= XP
    ,  PROV_RSA_AES       := 24 ; >= XP
    
    ;   CryptAcquireContext - dwFlags
    ;       http://msdn.microsoft.com/en-us/library/aa379886
static VERIFYCONTEXT  := 0xF0000000
    ,  NEWKEYSET      := 0x00000008
    ,  DELETEKEYSET   := 0x00000010
    ,  MACHINE_KEYSET := 0x00000020
    ,  SILENT         := 0x00000040
    ,  CRYPT_DEFAULT_CONTAINER_OPTIONAL := 0x00000080

    ;   CryptGenKey - dwFlag
    ;       http://msdn.microsoft.com/en-us/library/aa379941
static EXPORTABLE     := 0x00000001
    ,  USER_PROTECTED := 0x00000002
    ,  CREATE_SALT    := 0x00000004
    ,  UPDATE_KEY     := 0x00000008
    ,  NO_SALT        := 0x00000010
    ,  PREGEN         := 0x00000040
    ,  ARCHIVABLE     := 0x00004000
    ,  FORCE_KEY_PROTECTION_HIGH := 0x00008000
    
    ;   Key Types
static AT_KEYEXCHANGE := 1
    ,  AT_SIGNATURE   := 2
    
    ;
    ;   METHODS
    ;
    
    AcquireContext(Container, Provider, dwProvType, dwFlags)
    {
        if DllCall("Advapi32\CryptAcquireContext"
                , "ptr*", hProv
                , "ptr", Container ? &Container : 0
                , "ptr", Provider ? &Provider : 0
                , "uint", dwProvType
                , "uint", dwFlags)
        {
            if (dwFlags & this.DELETEKEYSET)
                ; Success, but hProv is invalid in this case.
                return 1
            ; Wrap it up so it'll be released at some point.
            return new this.Context(hProv)
        }
        return 0
    }
    
    IsSigned(FilePath)
    {
        return DllCall("Crypt32\CryptQueryObject"
            , "uint", CERT_QUERY_OBJECT_FILE := 1
            , "wstr", FilePath
            , "uint", CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED := 1<<10
            , "uint", CERT_QUERY_FORMAT_FLAG_BINARY := 2
            , "uint", 0
            , "uint*", dwEncoding
            , "uint*", dwContentType
            , "uint*", dwFormatType
            , "ptr", 0
            , "ptr", 0
            , "ptr", 0)
    }
    
    ;
    ; Error Detection
    ;
    __Get(name)
    {
        ListLines
        MsgBox 16,, Attempt to access invalid property Crypt.%name%.
        Pause
    }
    
    ;
    ;   CLASSES
    ;
    
    class _Handle
    {
        __New(handle)
        {
            this.h := handle
        }
        
        __Delete()
        {
            this.Dispose()
        }
    }
    
    class Context extends Crypt._Handle
    {
        GenerateKey(KeyType, KeyBitLength, dwFlags)
        {
            if DllCall("Advapi32\CryptGenKey"
                    , "ptr", this.h
                    , "uint", KeyType
                    , "uint", (KeyBitLength << 16) | dwFlags
                    , "ptr*", hKey)
            {
                global Crypt
                return new Crypt.Key(hKey)
            }
            return 0
        }
        
        CreateSelfSignCertificate(NameObject, StartTime, EndTime)
        {
            ctx := DllCall("Crypt32\CertCreateSelfSignCertificate"
                , "ptr", this.h
                , "ptr", IsObject(NameObject) ? NameObject.p : NameObject
                , "uint", 0, "ptr", 0, "ptr", 0
                , "ptr", IsObject(StartTime) ? StartTime.p : StartTime
                , "ptr", IsObject(EndTime) ? EndTime.p : EndTime
                , "ptr", 0, "ptr")
            global Cert
            return ctx ? new Cert.Context(ctx) : 0
        }
        
        Dispose()
        {
            if this.h && DllCall("Advapi32\CryptReleaseContext", "ptr", this.h, "uint", 0)
                this.h := 0
        }
    }
    
    class Key extends Crypt._Handle
    {
        Dispose()
        {
            if this.h && DllCall("Advapi32\CryptDestroyKey", "ptr", this.h)
                this.h := 0
        }
    }
}