#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    OpenSession = 0x1002,
    CloseSession = 0x1003,
    GetStorageIDs = 0x1004,
    GetStorageInfo = 0x1005,
    GetObjectHandles = 0x1007,
    GetObjectInfo = 0x1008,
    GetObject = 0x1009,
    DeleteObject = 0x100B,
    SendObjectInfo = 0x100C,
    SendObject = 0x100D,
    GetObjectPropValue = 0x9803,
    SetObjectPropValue = 0x9804,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseCode {
    Undefined = 0x2000,
    OK = 0x2001,
    GeneralError = 0x2002,
    SessionNotOpen = 0x2003,
    InvalidTransactionID = 0x2004,
    OperationNotSupported = 0x2005,
    ParameterNotSupported = 0x2006,
    IncompleteTransfer = 0x2007,
    InvalidStorageId = 0x2008,
    InvalidObjectHandle = 0x2009,
    DeviceBusy = 0x2019,
    InvalidParentObject = 0x201A,
    InvalidObjectPropCode = 0xA801,
    InvalidObjectPropFormat = 0xA802,
    InvalidObjectPropValue = 0xA803,
}

impl ResponseCode {
    pub fn from_u16(value: u16) -> Option<Self> {
        Some(match value {
            0x2000 => Self::Undefined,
            0x2001 => Self::OK,
            0x2002 => Self::GeneralError,
            0x2003 => Self::SessionNotOpen,
            0x2004 => Self::InvalidTransactionID,
            0x2005 => Self::OperationNotSupported,
            0x2006 => Self::ParameterNotSupported,
            0x2007 => Self::IncompleteTransfer,
            0x2008 => Self::InvalidStorageId,
            0x2009 => Self::InvalidObjectHandle,
            0x2019 => Self::DeviceBusy,
            0x201A => Self::InvalidParentObject,
            0xA801 => Self::InvalidObjectPropCode,
            0xA802 => Self::InvalidObjectPropFormat,
            0xA803 => Self::InvalidObjectPropValue,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerType {
    Command = 1,
    Data = 2,
    Response = 3,
    Event = 4,
}
