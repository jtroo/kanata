#requires AutoHotkey v2.0

EnableUIAccess(ExePath) {
  static CertName := "AutoHotkey"
  hStore := DllCall("Crypt32\CertOpenStore", "ptr",10 ; STORE_PROV_SYSTEM_W
    , "uint",0, "ptr",0, "uint",0x20000 ; SYSTEM_STORE_LOCAL_MACHINE
    , "wstr","Root", "ptr")
  if !hStore {
    throw OSError()
  }
  store := CertStore(hStore)
  cert := CertContext() ; Find or create certificate for signing.
  while (cert.ptr := DllCall("Crypt32\CertFindCertificateInStore", "ptr",hStore
      , "uint",0x10001 ; X509_ASN_ENCODING|PKCS_7_ASN_ENCODING
      , "uint",0, "uint",0x80007 ; FIND_SUBJECT_STR
      , "wstr", CertName, "ptr",cert.ptr, "ptr"))
    && !(DllCall("Crypt32\CryptAcquireCertificatePrivateKey"
      , "ptr",cert, "uint",5 ; CRYPT_ACQUIRE_CACHE_FLAG|CRYPT_ACQUIRE_COMPARE_KEY_FLAG
      , "ptr",0, "ptr*", 0, "uint*", &keySpec:=0, "ptr",0)
      && (keySpec & 2)) { ; AT_SIGNATURE ; Keep looking for a certificate with a private key.
  }
  if !cert.ptr {
    cert := EnableUIAccess_CreateCert(CertName, hStore)
  }
  EnableUIAccess_SetManifest(ExePath)             	; Set uiAccess attribute in manifest
  EnableUIAccess_SignFile(ExePath, cert, CertName)	; Sign the file (otherwise uiAccess attribute is ignored)
  return true
}

EnableUIAccess_SetManifest(ExePath) {
  xml := ComObject("Msxml2.DOMDocument")
  xml.async := false
  xml.setProperty("SelectionLanguage", "XPath")
  xml.setProperty("SelectionNamespaces"
    , "xmlns:v1='urn:schemas-microsoft-com:asm.v1' "
    . "xmlns:v3='urn:schemas-microsoft-com:asm.v3'")
  try {
    if !xml.loadXML(EnableUIAccess_ReadManifest(ExePath)) {
      throw Error("Invalid manifest")
    }
  } catch as e {
    throw Error("Error loading manifest from " ExePath,, e.Message "`n  @ " e.File ":" e.Line)
  }


  node := xml.selectSingleNode("/v1:assembly/v3:trustInfo/v3:security"
    .                          "/v3:requestedPrivileges/v3:requestedExecutionLevel")
  if !node ; Not AutoHotkey?
    throw Error("Manifest is missing required elements")

  node.setAttribute("uiAccess", "true")
  xml := RTrim(xml.xml, "`r`n")

  data := Buffer(StrPut(xml, "utf-8") - 1)
  StrPut(xml, data, "utf-8")

  if !(hupd := DllCall("BeginUpdateResource", "str",ExePath, "int",false))
    throw OSError()
  r := DllCall("UpdateResource", "ptr",hupd, "ptr",24, "ptr",1
          , "ushort", 1033, "ptr",data, "uint",data.size)

  ; Retry loop to work around file locks (especially by antivirus)
  for delay in [0, 100, 500, 1000, 3500] {
    Sleep delay
    if DllCall("EndUpdateResource", "ptr",hupd, "int",!r) || !r
      return
    if !(A_LastError = 5 || A_LastError = 110) ; ERROR_ACCESS_DENIED || ERROR_OPEN_FAILED
      break
  }
  throw OSError(A_LastError, "EndUpdateResource")
}

EnableUIAccess_ReadManifest(ExePath) {
  if !(hmod := DllCall("LoadLibraryEx", "str",ExePath, "ptr",0, "uint",2, "ptr"))
    throw OSError()
  try {
    if !(hres := DllCall("FindResource", "ptr",hmod, "ptr",1, "ptr",24, "ptr")) {
      throw OSError()
    }
    size := DllCall("SizeofResource", "ptr",hmod, "ptr",hres, "uint")
    if !(hglb := DllCall("LoadResource", "ptr",hmod, "ptr",hres, "ptr")) {
      throw OSError()
    }
    if !(pres := DllCall("LockResource", "ptr",hglb, "ptr")) {
      throw OSError()
    }
    return StrGet(pres, size, "utf-8")
  }
  finally {
    DllCall("FreeLibrary", "ptr",hmod)
  }
}

EnableUIAccess_CreateCert(Name, hStore) {
  prov := CryptContext() ; Here Name is used as the key container name.
  if !DllCall("Advapi32\CryptAcquireContext", "ptr*", prov
    , "str",Name, "ptr",0, "uint",1, "uint",0) { ; PROV_RSA_FULL=1, open existing=0
    if !DllCall("Advapi32\CryptAcquireContext", "ptr*", prov
      , "str",Name, "ptr",0, "uint",1, "uint",8) { ; PROV_RSA_FULL=1, CRYPT_NEWKEYSET=8
      throw OSError()
    }
    if !DllCall("Advapi32\CryptGenKey", "ptr",prov
        , "uint",2, "uint",0x4000001, "ptr*", CryptKey()) { ; AT_SIGNATURE=2, EXPORTABLE=..01
      throw OSError()
    }
  }

  ; Here Name is used as the certificate subject and name.
  Loop 2 {
    if A_Index = 1 {
      pbName := cbName := 0
    } else {
      bName := Buffer(cbName), pbName := bName.ptr
    }
    if !DllCall("Crypt32\CertStrToName", "uint",1, "str","CN=" Name
      , "uint",3, "ptr",0, "ptr",pbName, "uint*", &cbName, "ptr",0) ; X509_ASN_ENCODING=1, CERT_X500_NAME_STR=3
      throw OSError()
  }
  cnb := Buffer(2*A_PtrSize), NumPut("ptr",cbName, "ptr",pbName, cnb)

  ; Set expiry to 9999-01-01 12pm +0.
  NumPut("short", 9999, "sort", 1, "short", 5, "short", 1, "short", 12, endTime := Buffer(16, 0))

  StrPut("2.5.29.4", szOID_KEY_USAGE_RESTRICTION := Buffer(9),, "cp0")
  StrPut("2.5.29.37", szOID_ENHANCED_KEY_USAGE := Buffer(10),, "cp0")
  StrPut("1.3.6.1.5.5.7.3.3", szOID_PKIX_KP_CODE_SIGNING := Buffer(18),, "cp0")

  ; CERT_KEY_USAGE_RESTRICTION_INFO key_usage;
  key_usage := Buffer(6*A_PtrSize, 0)
  NumPut('ptr', 0, 'ptr', 0, 'ptr', 1, 'ptr', key_usage.ptr + 5*A_PtrSize, 'ptr', 0
    , 'uchar', (CERT_DATA_ENCIPHERMENT_KEY_USAGE := 0x10)
         | (CERT_DIGITAL_SIGNATURE_KEY_USAGE := 0x80), key_usage)

  ; CERT_ENHKEY_USAGE enh_usage;
  enh_usage := Buffer(3*A_PtrSize)
  NumPut("ptr",1, "ptr",enh_usage.ptr + 2*A_PtrSize, "ptr",szOID_PKIX_KP_CODE_SIGNING.ptr, enh_usage)

  key_usage_data := EncodeObject(szOID_KEY_USAGE_RESTRICTION, key_usage)
  enh_usage_data := EncodeObject(szOID_ENHANCED_KEY_USAGE, enh_usage)

  EncodeObject(structType, structInfo) {
    encoder := DllCall.Bind("Crypt32\CryptEncodeObject", "uint",X509_ASN_ENCODING := 1
      , "ptr",structType, "ptr",structInfo)
    if !encoder("ptr",0, "uint*", &enc_size := 0)
      throw OSError()
    enc_data := Buffer(enc_size)
    if !encoder("ptr",enc_data, "uint*", &enc_size)
      throw OSError()
    enc_data.Size := enc_size
    return enc_data
  }

  ; CERT_EXTENSION extension[2]; CERT_EXTENSIONS extensions;
  NumPut("ptr",szOID_KEY_USAGE_RESTRICTION.ptr, "ptr",true, "ptr",key_usage_data.size, "ptr",key_usage_data.ptr
    ,    "ptr",szOID_ENHANCED_KEY_USAGE.ptr   , "ptr",true, "ptr",enh_usage_data.size, "ptr",enh_usage_data.ptr
    , extension := Buffer(8*A_PtrSize))
  NumPut("ptr",2, "ptr",extension.ptr, extensions := Buffer(2*A_PtrSize))

  if !hCert := DllCall("Crypt32\CertCreateSelfSignCertificate"
    , "ptr",prov, "ptr",cnb, "uint",0, "ptr",0
    , "ptr",0, "ptr",0, "ptr",endTime, "ptr",extensions, "ptr") {
    throw OSError()
  }
  cert := CertContext(hCert)

  if !DllCall("Crypt32\CertAddCertificateContextToStore", "ptr",hStore
    , "ptr",hCert, "uint",1, "ptr",0) { ; STORE_ADD_NEW=1
    throw OSError()
  }

  return cert
}

EnableUIAccess_DeleteCertAndKey(Name) {
  ; This first call "acquires" the key container but also deletes it.
  DllCall("Advapi32\CryptAcquireContext", "ptr*", 0, "str",Name
    , "ptr",0, "uint",1, "uint",16) ; PROV_RSA_FULL=1, CRYPT_DELETEKEYSET=16
  if !hStore := DllCall("Crypt32\CertOpenStore", "ptr",10 ; STORE_PROV_SYSTEM_W
    , "uint",0, "ptr",0, "uint",0x20000 ; SYSTEM_STORE_LOCAL_MACHINE
    , "wstr", "Root", "ptr")
    throw OSError()
  store := CertStore(hStore)
  deleted := 0
  ; Multiple certificates might be created over time as keys become inaccessible
  while p := DllCall("Crypt32\CertFindCertificateInStore", "ptr",hStore
    , "uint",0x10001 ; X509_ASN_ENCODING|PKCS_7_ASN_ENCODING
    , "uint",0, "uint",0x80007 ; FIND_SUBJECT_STR
    , "wstr", Name, "ptr",0, "ptr") {
    if !DllCall("Crypt32\CertDeleteCertificateFromStore", "ptr",p) {
      throw OSError()
    }
    deleted++
  }
  return deleted
}

class Crypt {
  static IsSigned(FilePath) {
    return DllCall("Crypt32\CryptQueryObject"
      ,"uint" 	, CERT_QUERY_OBJECT_FILE := 1
      ,"wstr" 	, FilePath
      ,"uint" 	, CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED := 1<<10
      ,"uint" 	, CERT_QUERY_FORMAT_FLAG_BINARY := 2
      ,"uint" 	, 0
      ,"uint*"	, &dwEncoding:=0
      ,"uint*"	, &dwContentType:=0
      ,"uint*"	, &dwFormatType:=0
      ,"ptr"  	, 0
      ,"ptr"  	, 0
      ,"ptr"  	, 0)
  }
}
class CryptPtrBase {
  __new(p:=0) => this.ptr := p
  __delete() => this.ptr && this.Dispose()
}
class CryptContext extends CryptPtrBase {
  Dispose() => DllCall("Advapi32\CryptReleaseContext", "ptr",this, "uint",0)
}
class CertContext extends CryptPtrBase {
  Dispose() => DllCall("Crypt32\CertFreeCertificateContext", "ptr",this)
}
class CertStore extends CryptPtrBase {
  Dispose() => DllCall("Crypt32\CertCloseStore", "ptr",this, "uint",0)
}
class CryptKey extends CryptPtrBase {
  Dispose() => DllCall("Advapi32\CryptDestroyKey", "ptr",this)
}

EnableUIAccess_SignFile(ExePath, CertCtx, Name) {
  file_info := struct( ; SIGNER_FILE_INFO
    "ptr",A_PtrSize*3, "ptr",StrPtr(ExePath))
  dwIndex := Buffer(4, 0) ; DWORD
  subject_info := struct( ; SIGNER_SUBJECT_INFO
    "ptr",A_PtrSize*4, "ptr",dwIndex.ptr, "ptr",SIGNER_SUBJECT_FILE:=1,
    "ptr",file_info.ptr)
  cert_store_info := struct( ; SIGNER_CERT_STORE_INFO
    "ptr",A_PtrSize*4, "ptr",CertCtx.ptr, "ptr",SIGNER_CERT_POLICY_CHAIN:=2)
  cert_info := struct( ; SIGNER_CERT
    "uint",8+A_PtrSize*2, "uint",SIGNER_CERT_STORE:=2,
    "ptr",cert_store_info.ptr)
  authcode_attr := struct( ; SIGNER_ATTR_AUTHCODE
    "uint",8+A_PtrSize*3, "int",false, "ptr",true, "ptr",StrPtr(Name))
  sig_info := struct( ; SIGNER_SIGNATURE_INFO
    "uint",8+A_PtrSize*4, "uint",CALG_SHA1:=0x8004,
    "ptr",SIGNER_AUTHCODE_ATTR:=1, "ptr",authcode_attr.ptr)

  hr := DllCall("MSSign32\SignerSign"
    , "ptr",subject_info, "ptr",cert_info, "ptr",sig_info
    , "ptr",0, "ptr",0, "ptr",0, "ptr",0, "hresult") ; pProviderInfo pwszHttpTimeStamp psRequest pSipData

  struct(args*) => (
    args.Push(b := Buffer(args[2], 0)),
    NumPut(args*),
    b
  )
}

EnableUIAccess_Verify(ExePath) { ; Verifies a signed executable file.  Returns 0 on success, or a standard OS error number.
  wfi := Buffer(4*A_PtrSize) ; WINTRUST_FILE_INFO
  NumPut('ptr', wfi.size, 'ptr', StrPtr(ExePath), 'ptr', 0, 'ptr', 0, wfi)
  NumPut('int64', 0x11d0cd4400aac56b, 'int64', 0xee95c24fc000c28c, actionID := Buffer(16)) ; WINTRUST_ACTION_GENERIC_VERIFY_V2

  wtd := Buffer(9*A_PtrSize+16) ; WINTRUST_DATA
  NumPut(
    'ptr', wtd.Size, 'ptr', 0, 'ptr', 0, 'int', WTD_UI_NONE:=2, 'int', WTD_REVOKE_NONE:=0,
    'ptr', WTD_CHOICE_FILE:=1, 'ptr', wfi.ptr, 'ptr', WTD_STATEACTION_VERIFY:=1,
    'ptr', 0, 'ptr', 0, 'int', 0, 'int', 0, 'ptr', 0, wtd
  )
  return DllCall('wintrust\WinVerifyTrust', 'ptr', 0, 'ptr', actionID, 'ptr', wtd, 'int')
}
