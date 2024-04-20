class Cert
{
    ;   Encoding Types
static X509_ASN_ENCODING    := 0x00000001
    ,  PKCS_7_ASN_ENCODING  := 0x00010000

    ;   Certificate Information Flags (CERT_INFO_*)
static INFO_VERSION_FLAG                 := 1
    ,  INFO_SERIAL_NUMBER_FLAG           := 2
    ,  INFO_SIGNATURE_ALGORITHM_FLAG     := 3
    ,  INFO_ISSUER_FLAG                  := 4
    ,  INFO_NOT_BEFORE_FLAG              := 5
    ,  INFO_NOT_AFTER_FLAG               := 6
    ,  INFO_SUBJECT_FLAG                 := 7
    ,  INFO_SUBJECT_PUBLIC_KEY_INFO_FLAG := 8
    ,  INFO_ISSUER_UNIQUE_ID_FLAG        := 9
    ,  INFO_SUBJECT_UNIQUE_ID_FLAG       := 10
    ,  INFO_EXTENSION_FLAG               := 11

    ;   Certificate Comparison Functions (CERT_COMPARE_*)
static COMPARE_MASK                      := 0xFFFF
    ,  COMPARE_SHIFT                     := (_ := 16)
    ,  COMPARE_ANY                       := 0
    ,  COMPARE_SHA1_HASH                 := 1
    ,  COMPARE_NAME                      := 2
    ,  COMPARE_ATTR                      := 3
    ,  COMPARE_MD5_HASH                  := 4
    ,  COMPARE_PROPERTY                  := 5
    ,  COMPARE_PUBLIC_KEY                := 6
    ,  COMPARE_HASH                      := Cert.COMPARE_SHA1_HASH
    ,  COMPARE_NAME_STR_A                := 7
    ,  COMPARE_NAME_STR_W                := 8
    ,  COMPARE_KEY_SPEC                  := 9
    ,  COMPARE_ENHKEY_USAGE              := 10
    ,  COMPARE_CTL_USAGE                 := Cert.COMPARE_ENHKEY_USAGE
    ,  COMPARE_SUBJECT_CERT              := 11
    ,  COMPARE_ISSUER_OF                 := 12
    ,  COMPARE_EXISTING                  := 13
    ,  COMPARE_SIGNATURE_HASH            := 14
    ,  COMPARE_KEY_IDENTIFIER            := 15
    ,  COMPARE_CERT_ID                   := 16
    ,  COMPARE_CROSS_CERT_DIST_POINTS    := 17
    ,  COMPARE_PUBKEY_MD5_HASH           := 18
    ,  COMPARE_SUBJECT_INFO_ACCESS       := 19

    ;   dwFindType Flags (CERT_FIND_*)
static FIND_ANY                    := Cert.COMPARE_ANY            << _
    ,  FIND_SHA1_HASH              := Cert.COMPARE_SHA1_HASH      << _
    ,  FIND_MD5_HASH               := Cert.COMPARE_MD5_HASH       << _
    ,  FIND_SIGNATURE_HASH         := Cert.COMPARE_SIGNATURE_HASH << _
    ,  FIND_KEY_IDENTIFIER         := Cert.COMPARE_KEY_IDENTIFIER << _
    ,  FIND_HASH                   := Cert.FIND_SHA1_HASH
    ,  FIND_PROPERTY               := Cert.COMPARE_PROPERTY       << _
    ,  FIND_PUBLIC_KEY             := Cert.COMPARE_PUBLIC_KEY     << _
    ,  FIND_SUBJECT_NAME           := (Cert.COMPARE_NAME       << _) | Cert.INFO_SUBJECT_FLAG
    ,  FIND_SUBJECT_ATTR           := (Cert.COMPARE_ATTR       << _) | Cert.INFO_SUBJECT_FLAG
    ,  FIND_ISSUER_NAME            := (Cert.COMPARE_NAME       << _) | Cert.INFO_ISSUER_FLAG
    ,  FIND_ISSUER_ATTR            := (Cert.COMPARE_ATTR       << _) | Cert.INFO_ISSUER_FLAG
    ,  FIND_SUBJECT_STR            := (Cert.COMPARE_NAME_STR_W << _) | Cert.INFO_SUBJECT_FLAG
    ,  FIND_ISSUER_STR             := (Cert.COMPARE_NAME_STR_W << _) | Cert.INFO_ISSUER_FLAG
    ,  FIND_KEY_SPEC               := Cert.COMPARE_KEY_SPEC       << _
    ,  FIND_ENHKEY_USAGE           := Cert.COMPARE_ENHKEY_USAGE   << _
    ,  FIND_CTL_USAGE              := Cert.FIND_ENHKEY_USAGE
    ,  FIND_SUBJECT_CERT           := Cert.COMPARE_SUBJECT_CERT   << _
    ,  FIND_ISSUER_OF              := Cert.COMPARE_ISSUER_OF      << _
    ,  FIND_EXISTING               := Cert.COMPARE_EXISTING       << _
    ,  FIND_CERT_ID                := Cert.COMPARE_CERT_ID        << _
    ,  FIND_CROSS_CERT_DIST_POINTS := Cert.COMPARE_CROSS_CERT_DIST_POINTS << _
    ,  FIND_PUBKEY_MD5_HASH        := Cert.COMPARE_PUBKEY_MD5_HASH        << _
    ,  FIND_SUBJECT_INFO_ACCESS    := Cert.COMPARE_SUBJECT_INFO_ACCESS    << _

    ;   Certificate Store Provider Types (CERT_STORE_PROV_*)
static STORE_PROV_MSG                 := 1
    ,  STORE_PROV_MEMORY              := 2
    ,  STORE_PROV_FILE                := 3
    ,  STORE_PROV_REG                 := 4
    ,  STORE_PROV_PKCS7               := 5
    ,  STORE_PROV_SERIALIZED          := 6
    ,  STORE_PROV_FILENAME_A          := 7
    ,  STORE_PROV_FILENAME_W          := 8
    ,  STORE_PROV_FILENAME            := Cert.STORE_PROV_FILENAME_W
    ,  STORE_PROV_SYSTEM_A            := 9
    ,  STORE_PROV_SYSTEM_W            := 10
    ,  STORE_PROV_SYSTEM              := Cert.STORE_PROV_SYSTEM_W
    ,  STORE_PROV_COLLECTION          := 11
    ,  STORE_PROV_SYSTEM_REGISTRY_A   := 12
    ,  STORE_PROV_SYSTEM_REGISTRY_W   := 13
    ,  STORE_PROV_SYSTEM_REGISTRY     := Cert.STORE_PROV_SYSTEM_REGISTRY_W
    ,  STORE_PROV_PHYSICAL_W          := 14
    ,  STORE_PROV_PHYSICAL            := Cert.STORE_PROV_PHYSICAL_W
    ,  STORE_PROV_LDAP_W              := 16
    ,  STORE_PROV_LDAP                := Cert.STORE_PROV_LDAP_W
    ,  STORE_PROV_PKCS12              := 17

    ;   Certificate Store open/property flags (low-word; CERT_STORE_*)
static STORE_NO_CRYPT_RELEASE_FLAG            := 0x0001
    ,  STORE_SET_LOCALIZED_NAME_FLAG          := 0x0002
    ,  STORE_DEFER_CLOSE_UNTIL_LAST_FREE_FLAG := 0x0004
    ,  STORE_DELETE_FLAG                      := 0x0010
    ,  STORE_UNSAFE_PHYSICAL_FLAG             := 0x0020
    ,  STORE_SHARE_STORE_FLAG                 := 0x0040
    ,  STORE_SHARE_CONTEXT_FLAG               := 0x0080
    ,  STORE_MANIFOLD_FLAG                    := 0x0100
    ,  STORE_ENUM_ARCHIVED_FLAG               := 0x0200
    ,  STORE_UPDATE_KEYID_FLAG                := 0x0400
    ,  STORE_BACKUP_RESTORE_FLAG              := 0x0800
    ,  STORE_READONLY_FLAG                    := 0x8000
    ,  STORE_OPEN_EXISTING_FLAG               := 0x4000
    ,  STORE_CREATE_NEW_FLAG                  := 0x2000
    ,  STORE_MAXIMUM_ALLOWED_FLAG             := 0x1000

    ;   Certificate System Store Flag Values (high-word; CERT_SYSTEM_STORE_*)
static SYSTEM_STORE_MASK                          := 0xFFFF0000
    ,  SYSTEM_STORE_RELOCATE_FLAG                 := 0x80000000
    ,  SYSTEM_STORE_UNPROTECTED_FLAG              := 0x40000000
    ; Location of the system store:
    ,  SYSTEM_STORE_LOCATION_MASK                 := 0x00FF0000
    ,  SYSTEM_STORE_LOCATION_SHIFT                := (_ := 16)
    ; Registry: HKEY_CURRENT_USER or HKEY_LOCAL_MACHINE
    ,  SYSTEM_STORE_CURRENT_USER_ID               := 1
    ,  SYSTEM_STORE_LOCAL_MACHINE_ID              := 2
    ; Registry: HKEY_LOCAL_MACHINE\Software\Microsoft\Cryptography\Services
    ,  SYSTEM_STORE_CURRENT_SERVICE_ID            := 4
    ,  SYSTEM_STORE_SERVICES_ID                   := 5
    ; Registry: HKEY_USERS
    ,  SYSTEM_STORE_USERS_ID                      := 6
    ; Registry: HKEY_CURRENT_USER\Software\Policies\Microsoft\SystemCertificates
    ,  SYSTEM_STORE_CURRENT_USER_GROUP_POLICY_ID  := 7
    ; Registry: HKEY_LOCAL_MACHINE\Software\Policies\Microsoft\SystemCertificates
    ,  SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY_ID := 8
    ; Registry: HKEY_LOCAL_MACHINE\Software\Microsoft\EnterpriseCertificates
    ,  SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE_ID   := 9
    ,  SYSTEM_STORE_CURRENT_USER               := (Cert.SYSTEM_STORE_CURRENT_USER_ID               << _)
    ,  SYSTEM_STORE_LOCAL_MACHINE              := (Cert.SYSTEM_STORE_LOCAL_MACHINE_ID              << _)
    ,  SYSTEM_STORE_CURRENT_SERVICE            := (Cert.SYSTEM_STORE_CURRENT_SERVICE_ID            << _)
    ,  SYSTEM_STORE_SERVICES                   := (Cert.SYSTEM_STORE_SERVICES_ID                   << _)
    ,  SYSTEM_STORE_USERS                      := (Cert.SYSTEM_STORE_USERS_ID                      << _)
    ,  SYSTEM_STORE_CURRENT_USER_GROUP_POLICY  := (Cert.SYSTEM_STORE_CURRENT_USER_GROUP_POLICY_ID  << _)
    ,  SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY := (Cert.SYSTEM_STORE_LOCAL_MACHINE_GROUP_POLICY_ID << _)
    ,  SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE   := (Cert.SYSTEM_STORE_LOCAL_MACHINE_ENTERPRISE_ID   << _)

    ;   Certificate name types (CERT_NAME_*)
static NAME_EMAIL_TYPE            := 1
    ,  NAME_RDN_TYPE              := 2
    ,  NAME_ATTR_TYPE             := 3
    ,  NAME_SIMPLE_DISPLAY_TYPE   := 4
    ,  NAME_FRIENDLY_DISPLAY_TYPE := 5
    ,  NAME_DNS_TYPE              := 6
    ,  NAME_URL_TYPE              := 7
    ,  NAME_UPN_TYPE              := 8
    ; Certificate name flags
    ,  NAME_ISSUER_FLAG              := 0x00000001
    ,  NAME_DISABLE_IE4_UTF8_FLAG    := 0x00010000
    ,  NAME_STR_ENABLE_PUNYCODE_FLAG := 0x00200000

    ;   dwAddDisposition values (CERT_STORE_ADD_*)
static STORE_ADD_NEW                                 := 1
    ,  STORE_ADD_USE_EXISTING                        := 2
    ,  STORE_ADD_REPLACE_EXISTING                    := 3
    ,  STORE_ADD_ALWAYS                              := 4
    ,  STORE_ADD_REPLACE_EXISTING_INHERIT_PROPERTIES := 5
    ,  STORE_ADD_NEWER                               := 6
    ,  STORE_ADD_NEWER_INHERIT_PROPERTIES            := 7

    ;
    ; Static Methods
    ;

    OpenStore(pStoreProvider, dwMsgAndCertEncodingType, dwFlags, ParamType="Ptr", Param=0)
    {
        hCertStore := DllCall("Crypt32\CertOpenStore"
            , "ptr", pStoreProvider
            , "uint", dwMsgAndCertEncodingType
            , "ptr", 0 ; hCryptProv
            , "uint", dwFlags
            , ParamType, Param)
        if hCertStore
            hCertStore := new this.Store(hCertStore)
        return hCertStore
    }

    GetStoreNames(dwFlags)
    {
        static cb := RegisterCallback("Cert_GetStoreNames_Callback", "F")
        global Cert
        DllCall("Crypt32\CertEnumSystemStore", "uint", dwFlags
                , "ptr", 0, "ptr", &(names := []), "ptr", cb)
        return names
    }


    ;
    ;   Certificate Name
    ;
    class Name
    {
        __New(Props)
        {
            static Fields := {
            (Join,
                CommonName:         "CN"
                LocalityName:       "L"
                Organization:       "O"
                OrganizationalUnit: "OU"
                Email:              "E"
                Country:            "C"
                State:              "ST"
                StreetAddress:      "STREET"
                Title:              "T"
                GivenName:          "G"
                Initials:           "I"
                Surname:            "SN"
                Doman:              "DC"
            )}
            static CERT_X500_NAME_STR := 3, Q := """" ; For readability.
            
            if IsObject(Props)
            {
                ; Build name string from caller-supplied object.
                name_string := ""
                for field_name, field_code in Fields
                {
                    if Props.HasKey(field_name)
                    {
                        if (name_string != "")
                            name_string .= ";"
                        name_string .= field_code "=" Q RegExReplace(Props[field_name], Q, Q Q) Q
                    }
                }
            }
            else
                name_string := Props
            
            Loop 2
            {
                if A_Index=1
                {   ; First iteration: retrieve required size.
                    pbEncoded := 0
                    cbEncoded := 0
                }
                else
                {   ; Second iteration: retrieve encoded name.
                    this.SetCapacity("data", cbEncoded)
                    pbEncoded := this.GetAddress("data")
                }
                global Cert
                if !DllCall("Crypt32\CertStrToName"
                    , "uint", Cert.X509_ASN_ENCODING
                    , "str", name_string
                    , "uint", CERT_X500_NAME_STR
                    , "ptr", 0 ; Reserved
                    , "ptr", pbEncoded
                    , "uint*", cbEncoded
                    , "str*", ErrorString)
                {
                    ErrorLevel := ErrorString
                    return false
                }
            }
            this.SetCapacity("blob", A_PtrSize*2)  ; CERT_NAME_BLOB
            NumPut(pbEncoded, NumPut(cbEncoded, this.p := this.GetAddress("blob")))
        }
    }
    
    
    ;
    ;   Certificate Store
    ;
    class Store
    {
        FindCertificates(dwFindFlags, dwFindType, FindParamType="ptr", FindParam=0)
        {
            global Cert
            hStore := this.h
            , dwCertEncodingType := Cert.X509_ASN_ENCODING | Cert.PKCS_7_ASN_ENCODING
            , ctx := new Cert.Context(0)
            , certs := []
            while ctx.p := DllCall("Crypt32\CertFindCertificateInStore"
                , "ptr", hStore
                , "uint", dwCertEncodingType
                , "uint", dwFindFlags
                , "uint", dwFindType
                , FindParamType, FindParam
                , "ptr", ctx.p  ; If non-NULL, this context is freed.
                , "ptr")
            {
                ; Each certificate context must be duplicated since the next
                ; call will free it.
                certs.Insert(ctx.Duplicate())
            }
            ctx.p := 0 ; Above freed it already.
            return certs
        }
        
        AddCertificate(Certificate, dwAddDisposition)
        {
            if !DllCall("Crypt32\CertAddCertificateContextToStore"
                    , "ptr", this.h
                    , "ptr", Certificate.p
                    , "uint", dwAddDisposition
                    , "ptr*", pStoreContext)
                return 0
            global Cert
            return pStoreContext ? new Cert.Context(pStoreContext) : 0
        }
        
        __New(handle)
        {
            this.h := handle
        }
        
        __Delete()
        {
            if this.h && DllCall("Crypt32\CertCloseStore", "ptr", this.h, "uint", 0)
                this.h := 0
        }
        
        static Dispose := Cert.Store.__Delete ; Alias
    }


    ;
    ;   Certificate Context
    ;
    class Context
    {
        __New(ptr)
        {
            this.p := ptr
        }
        
        __Delete()
        {
            if this.p && DllCall("Crypt32\CertFreeCertificateContext", "ptr", this.p)
                this.p := 0
        }
        
        ; CertGetNameString
        ;   http://msdn.microsoft.com/en-us/library/aa376086
        GetNameString(dwType, dwFlags=0, pvTypePara=0)
        {
            if !this.p
                return
            cc := DllCall("Crypt32\CertGetNameString", "ptr", this.p, "uint", dwType, "uint", dwFlags, "ptr", pvTypePara, "ptr", 0, "uint", 0)
            if cc <= 1 ; i.e. empty string.
                return
            VarSetCapacity(name, cc*2)
            DllCall("Crypt32\CertGetNameString", "ptr", this.p, "uint", dwType, "uint", dwFlags, "ptr", pvTypePara, "str", name, "uint", cc)
            return name
        }
        
        ; CertDuplicateCertificateContext
        ;   http://msdn.microsoft.com/en-us/library/aa376045
        Duplicate()
        {
            return this.p && (p := DllCall("Crypt32\CertDuplicateCertificateContext", "ptr", this.p))
                ? new this.base(p) : p
        }
        
        static Dispose := Cert.Context.__Delete ; Alias
    }


    ;
    ; Error Detection
    ;
    __Get(name)
    {
        ListLines
        MsgBox 16,, Attempt to access invalid property Cert.%name%.
        Pause
    }
}


;
; Internal
;

Cert_GetStoreNames_Callback(pvSystemStore, dwFlags, pStoreInfo, pvReserved, pvArg)
{
    Object(pvArg).Insert(StrGet(pvSystemStore, "utf-16"))
    return true
}