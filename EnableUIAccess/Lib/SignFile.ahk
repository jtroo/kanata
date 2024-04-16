SignFile(File, CertCtx, Name)
{
    VarSetCapacity(wfile, 2 * StrPut(File, "utf-16")), StrPut(File, &wfile, "utf-16")
    VarSetCapacity(wname, 2 * StrPut(Name, "utf-16")), StrPut(Name, &wname, "utf-16")
    cert_ptr := IsObject(CertCtx) ? CertCtx.p : CertCtx

    VarSetCapacity(file_info,       A_PtrSize*3, 0) ; SIGNER_FILE_INFO
    NumPut(3*A_PtrSize,                 file_info, 0)
    NumPut(&wfile,                      file_info, A_PtrSize)

    VarSetCapacity(dwIndex, 4, 0)

    VarSetCapacity(subject_info,    A_PtrSize*4, 0) ; SIGNER_SUBJECT_INFO
    NumPut(A_PtrSize*4,                 subject_info, 0)
    NumPut(&dwIndex,                    subject_info, A_PtrSize)  ; MSDN: "must be set to zero" in this case means "must be set to the address of a field containing zero".
    NumPut(SIGNER_SUBJECT_FILE:=1,      subject_info, A_PtrSize*2)
    NumPut(&file_info,                  subject_info, A_PtrSize*3)

    VarSetCapacity(cert_store_info, A_PtrSize*4, 0) ; SIGNER_CERT_STORE_INFO
    NumPut(A_PtrSize*4,                 cert_store_info, 0)
    NumPut(cert_ptr,                    cert_store_info, A_PtrSize)
    NumPut(SIGNER_CERT_POLICY_CHAIN:=2, cert_store_info, A_PtrSize*3)

    VarSetCapacity(cert_info,       8+A_PtrSize*2, 0) ; SIGNER_CERT
    NumPut(8+A_PtrSize*2,               cert_info, 0, "uint")
    NumPut(SIGNER_CERT_STORE:=2,        cert_info, 4, "uint")
    NumPut(&cert_store_info,            cert_info, 8)

    VarSetCapacity(authcode_attr,   8+A_PtrSize*3, 0) ; SIGNER_ATTR_AUTHCODE
    NumPut(8+A_PtrSize*3,               authcode_attr, 0, "uint")
    NumPut(false,                       authcode_attr, 4, "int")  ; fCommercial
    NumPut(true,                        authcode_attr, 8)         ; fIndividual
    NumPut(&wname,                      authcode_attr, 8+A_PtrSize)

    VarSetCapacity(sig_info,        8+A_PtrSize*4, 0) ; SIGNER_SIGNATURE_INFO
    NumPut(8+A_PtrSize*4,               sig_info, 0, "uint")
    NumPut(CALG_SHA1:=0x8004,           sig_info, 4, "uint")
    NumPut(SIGNER_AUTHCODE_ATTR:=1,     sig_info, 8)
    NumPut(&authcode_attr,              sig_info, 8+A_PtrSize)

    hr := DllCall("MSSign32\SignerSign"
        , "ptr", &subject_info
        , "ptr", &cert_info
        , "ptr", &sig_info
        , "ptr", 0 ; pProviderInfo
        , "ptr", 0 ; pwszHttpTimeStamp
        , "ptr", 0 ; psRequest
        , "ptr", 0 ; pSipData
        , "uint")
    
    return 0 == (ErrorLevel := hr)
}