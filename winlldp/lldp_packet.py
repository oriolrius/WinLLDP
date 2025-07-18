import struct
from typing import List, Tuple, Optional, Dict, Any
from enum import IntEnum


class TLVType(IntEnum):
    END_OF_LLDPDU = 0
    CHASSIS_ID = 1
    PORT_ID = 2
    TTL = 3
    PORT_DESCRIPTION = 4
    SYSTEM_NAME = 5
    SYSTEM_DESCRIPTION = 6
    SYSTEM_CAPABILITIES = 7
    MANAGEMENT_ADDRESS = 8
    ORGANIZATIONALLY_SPECIFIC = 127


class ChassisIdSubtype(IntEnum):
    CHASSIS_COMPONENT = 1
    INTERFACE_ALIAS = 2
    PORT_COMPONENT = 3
    MAC_ADDRESS = 4
    NETWORK_ADDRESS = 5
    INTERFACE_NAME = 6
    LOCALLY_ASSIGNED = 7


class PortIdSubtype(IntEnum):
    INTERFACE_ALIAS = 1
    PORT_COMPONENT = 2
    MAC_ADDRESS = 3
    NETWORK_ADDRESS = 4
    INTERFACE_NAME = 5
    AGENT_CIRCUIT_ID = 6
    LOCALLY_ASSIGNED = 7


class TLV:
    def __init__(self, tlv_type: TLVType, value: bytes):
        self.type = tlv_type
        self.value = value
        self.length = len(value)

    def encode(self) -> bytes:
        if self.length > 511:
            raise ValueError(f"TLV length {self.length} exceeds maximum of 511")
        
        type_length = (self.type << 9) | self.length
        return struct.pack('!H', type_length) + self.value

    @classmethod
    def decode(cls, data: bytes, offset: int = 0) -> Tuple[Optional['TLV'], int]:
        if len(data) < offset + 2:
            return None, offset
        
        type_length = struct.unpack('!H', data[offset:offset + 2])[0]
        tlv_type = TLVType((type_length >> 9) & 0x7F)
        length = type_length & 0x1FF
        
        if len(data) < offset + 2 + length:
            return None, offset
        
        value = data[offset + 2:offset + 2 + length]
        return cls(tlv_type, value), offset + 2 + length


class LLDPPacket:
    def __init__(self):
        self.tlvs: List[TLV] = []

    def add_tlv(self, tlv: TLV):
        self.tlvs.append(tlv)

    def add_chassis_id(self, subtype: ChassisIdSubtype, value: bytes):
        tlv_value = struct.pack('!B', subtype) + value
        self.add_tlv(TLV(TLVType.CHASSIS_ID, tlv_value))

    def add_port_id(self, subtype: PortIdSubtype, value: bytes):
        tlv_value = struct.pack('!B', subtype) + value
        self.add_tlv(TLV(TLVType.PORT_ID, tlv_value))

    def add_ttl(self, seconds: int):
        self.add_tlv(TLV(TLVType.TTL, struct.pack('!H', seconds)))

    def add_port_description(self, description: str):
        self.add_tlv(TLV(TLVType.PORT_DESCRIPTION, description.encode('utf-8')))

    def add_system_name(self, name: str):
        self.add_tlv(TLV(TLVType.SYSTEM_NAME, name.encode('utf-8')))

    def add_system_description(self, description: str):
        self.add_tlv(TLV(TLVType.SYSTEM_DESCRIPTION, description.encode('utf-8')))

    def add_system_capabilities(self, capabilities: int, enabled: int):
        value = struct.pack('!HH', capabilities, enabled)
        self.add_tlv(TLV(TLVType.SYSTEM_CAPABILITIES, value))

    def add_management_address(self, address_type: int, address: bytes, 
                              interface_number: int, oid: bytes = b''):
        addr_len = len(address) + 1
        value = struct.pack('!B', addr_len) + struct.pack('!B', address_type) + address
        value += struct.pack('!B', 2)  # Interface numbering subtype
        value += struct.pack('!I', interface_number)
        value += struct.pack('!B', len(oid)) + oid
        self.add_tlv(TLV(TLVType.MANAGEMENT_ADDRESS, value))

    def add_organizationally_specific(self, oui: bytes, subtype: int, info: bytes):
        """Add an organizationally specific TLV"""
        value = oui + struct.pack('!B', subtype) + info
        self.add_tlv(TLV(TLVType.ORGANIZATIONALLY_SPECIFIC, value))

    def add_end_of_lldpdu(self):
        self.add_tlv(TLV(TLVType.END_OF_LLDPDU, b''))

    def encode(self) -> bytes:
        packet = b''
        for tlv in self.tlvs:
            packet += tlv.encode()
        return packet

    @classmethod
    def decode(cls, data: bytes) -> 'LLDPPacket':
        packet = cls()
        offset = 0
        
        while offset < len(data):
            tlv, new_offset = TLV.decode(data, offset)
            if tlv is None:
                break
            
            packet.add_tlv(tlv)
            offset = new_offset
            
            if tlv.type == TLVType.END_OF_LLDPDU:
                break
        
        return packet

    def get_tlv_value(self, tlv_type: TLVType) -> Optional[bytes]:
        for tlv in self.tlvs:
            if tlv.type == tlv_type:
                return tlv.value
        return None

    def to_dict(self) -> Dict[str, Any]:
        result = {}
        
        for tlv in self.tlvs:
            if tlv.type == TLVType.CHASSIS_ID:
                subtype = struct.unpack('!B', tlv.value[0:1])[0]
                value = tlv.value[1:]
                if subtype == ChassisIdSubtype.MAC_ADDRESS:
                    result['chassis_id'] = ':'.join(f'{b:02x}' for b in value)
                else:
                    result['chassis_id'] = value.decode('utf-8', errors='ignore')
                result['chassis_id_subtype'] = ChassisIdSubtype(subtype).name
                
            elif tlv.type == TLVType.PORT_ID:
                subtype = struct.unpack('!B', tlv.value[0:1])[0]
                value = tlv.value[1:]
                if subtype == PortIdSubtype.MAC_ADDRESS:
                    result['port_id'] = ':'.join(f'{b:02x}' for b in value)
                else:
                    result['port_id'] = value.decode('utf-8', errors='ignore')
                result['port_id_subtype'] = PortIdSubtype(subtype).name
                
            elif tlv.type == TLVType.TTL:
                result['ttl'] = struct.unpack('!H', tlv.value)[0]
                
            elif tlv.type == TLVType.PORT_DESCRIPTION:
                result['port_description'] = tlv.value.decode('utf-8', errors='ignore')
                
            elif tlv.type == TLVType.SYSTEM_NAME:
                result['system_name'] = tlv.value.decode('utf-8', errors='ignore')
                
            elif tlv.type == TLVType.SYSTEM_DESCRIPTION:
                result['system_description'] = tlv.value.decode('utf-8', errors='ignore')
                
            elif tlv.type == TLVType.SYSTEM_CAPABILITIES:
                caps, enabled = struct.unpack('!HH', tlv.value)
                result['system_capabilities'] = caps
                result['enabled_capabilities'] = enabled
                
            elif tlv.type == TLVType.MANAGEMENT_ADDRESS:
                addr_len = struct.unpack('!B', tlv.value[0:1])[0]
                addr_type = struct.unpack('!B', tlv.value[1:2])[0]
                address = tlv.value[2:1 + addr_len]
                if addr_type == 1:  # IPv4
                    result['management_address'] = '.'.join(str(b) for b in address)
                else:
                    result['management_address'] = ':'.join(f'{b:02x}' for b in address)
        
        return result